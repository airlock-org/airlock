//! Rust owns memory-accounting scenarios while the actual patched C header performs reporter arithmetic.

use crate::common;
use anyhow::Result;
use libloading::Library;
use std::ffi::{CString, c_char, c_int, c_long};

/// One prepared C ABI library, retained while any copied function pointer can be called.
struct Accounting {
    // Keeps the trusted fixture implementation mapped for the lifetime of the function pointers.
    _library: Library,
    // Parses a NUL-terminated /proc/zoneinfo snapshot into the caller's writable output cell.
    read: unsafe extern "C" fn(*const c_char, c_long, *mut i64) -> c_int,
    // Checks whether one before/after sample pair stays within the reporter's retry threshold.
    consistent: unsafe extern "C" fn(i64, i64, c_int) -> c_int,
    // Computes demand from a complete seven-field snapshot without overflowing its result.
    used: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, *mut i64) -> c_int,
}

impl Accounting {
    /// Load only the explicitly prepared fixture library; quick execution never invokes a compiler.
    fn new() -> Result<Self> {
        let path = common::binary("AIRLOCK_MEMORY_TEST_LIBRARY", "memory-accounting.so");
        // The preparation command compiles this checked-in wrapper with the checked-in upstream header.
        // Each symbol has exactly the C declaration above, and the owned library outlives every call.
        unsafe {
            let library = Library::new(path)?;
            let read = *library.get(b"read_pcp\0")?;
            let consistent = *library.get(b"consistent\0")?;
            let used = *library.get(b"used\0")?;
            Ok(Self {
                _library: library,
                read,
                consistent,
                used,
            })
        }
    }

    /// Supply a terminated input string and one valid writable result cell to the C parser.
    fn read(&self, text: &str, page_size: c_long, initial: i64) -> Result<(bool, i64)> {
        let text = CString::new(text)?;
        let mut result = initial;
        // CString guarantees termination; C only writes through this nonnull, uniquely borrowed cell.
        let accepted = unsafe { (self.read)(text.as_ptr(), page_size, &mut result) } != 0;
        Ok((accepted, result))
    }

    /// Call pure integer arithmetic through the exact compiled C ABI.
    fn used(&self, values: [i64; 7]) -> (bool, i64) {
        let mut result = -1;
        // All arguments are values, and the output pointer names one initialized, uniquely borrowed cell.
        let accepted = unsafe {
            (self.used)(
                values[0],
                values[1],
                values[2],
                values[3],
                values[4],
                values[5],
                values[6],
                &mut result,
            )
        } != 0;
        (accepted, result)
    }
}

/// Generate the kernel's per-zone, per-CPU pageset shape without counting its high-water limits.
fn zone(counts: &[&str], name: &str) -> String {
    let mut result = format!("Node 0, zone {name}\n  pages free 123\n  pagesets\n");
    for (cpu, count) in counts.iter().enumerate() {
        result.push_str(&format!("    cpu: {cpu}\n              count: {count}\n              high: 999999\n              batch: 63\n"));
    }
    result.push_str("  vm stats threshold: 32\n  node_unreclaimable: 0\n");
    result
}

/// Sum pagesets across zones and CPUs while preserving demand as pages move between free lists.
#[test]
fn memory_accounting_preserves_demand_and_swap_semantics() -> Result<()> {
    let accounting = Accounting::new()?;
    for page_size in [4096, 16384, 65536] {
        let text = zone(&["0", "17"], "DMA32") + &zone(&["2000", "3000"], "Normal");
        assert_eq!(
            accounting.read(&text, page_size, -1)?,
            (true, 5017 * (page_size / 1024))
        );
    }
    for pcp in [0, 4096, 128000, 320000, 500000, 128000, 0] {
        assert_eq!(
            accounting.used([1048576, 600000 - pcp, pcp, 0, 65536, 0, 0]),
            (true, 383040)
        );
    }
    for (values, expected) in [
        ([1048576, 500000, 0, 0, 65536, 0, 0], 483040),
        ([1248576, 700000, 100000, 0, 65536, 0, 0], 383040),
        ([1048576, 600000, 0, 0, 75536, 0, 0], 373040),
        (
            [1000000, 400000, 100000, 10000, 50000, 200000, 170000],
            470000,
        ),
        ([i64::MAX, 0, 0, 0, 0, i64::MAX, i64::MAX], i64::MAX),
    ] {
        assert_eq!(accounting.used(values), (true, expected));
    }
    Ok(())
}

/// Partial pagesets, overflow and malformed records never publish a partial correction.
#[test]
fn memory_accounting_rejects_damaged_samples_without_modifying_outputs() -> Result<()> {
    let accounting = Accounting::new()?;
    let ordinary = zone(&["0"], "Normal");
    let mut damaged = vec![
        String::new(),
        "Node 0, zone Normal\n".into(),
        "pagesets\n    cpu: 0\n".into(),
        ordinary.replace("count: 0", "high: 0"),
        ordinary.replace("cpu: 0\n", "cpu: 0\n    cpu: 1\n"),
        ordinary.replace("count: 0\n", "count: 0\n              count: 1\n"),
        ordinary.trim_end_matches('\n').into(),
        zone(&["17"], "Normal") + "Node 1, zone Normal\n pagesets\n cpu: 0\n count: bad\n",
    ];
    for counts in [
        vec!["bad"],
        vec!["-1"],
        vec!["12garbage"],
        vec!["18446744073709551616"],
        vec!["18446744073709551615", "1"],
        vec!["9223372036854775807"],
    ] {
        damaged.push(zone(&counts, "Normal"));
    }
    for text in damaged {
        assert_eq!(
            accounting.read(&text, 4096, 123456)?,
            (false, 123456),
            "{text}"
        );
    }
    for page_size in [-1, 0, 512, 4097] {
        assert!(!accounting.read(&ordinary, page_size, -1)?.0);
    }
    for values in [
        [100, 50, 60, 0, 0, 0, 0],
        [100, 0, 0, 0, 0, 0, 1],
        [100, -1, 0, 0, 0, 0, 0],
        [i64::MAX, 0, 0, 0, 0, 1, 0],
    ] {
        assert_eq!(accounting.used(values), (false, -1));
    }
    for (before, after, expected) in [
        (100000, 100000, true),
        (100000, 115000, true),
        (115000, 100000, true),
        (100000, 115001, false),
        (500000, 100000, false),
        (-1, 0, false),
    ] {
        // The loaded function consumes only integer values and cannot retain borrowed Rust memory.
        assert_eq!(
            unsafe { (accounting.consistent)(before, after, 30000) } != 0,
            expected
        );
    }
    Ok(())
}
