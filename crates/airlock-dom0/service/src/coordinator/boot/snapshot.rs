//! Compare saved Qubes XML and disk metadata before reusing an applied boot receipt.

use super::*;
use quick_xml::{Reader, events::Event};
use std::io::Read;

#[derive(Clone)]
struct Node {
    // XML element name from Qubes' saved configuration.
    tag: String,
    // Unescaped attribute values, sorted independently of XML formatting.
    attrs: BTreeMap<String, String>,
    // Trimmed element text; formatting whitespace carries no configuration meaning.
    text: String,
    // Ordered XML children before canonicalization and runtime-field filtering.
    children: Vec<Node>,
}

impl Node {
    fn child(&self, tag: &str) -> Result<&Node> {
        self.children
            .iter()
            .find(|n| n.tag == tag)
            .context("incomplete Qubes configuration; use recovery")
    }

    fn attr(&self, key: &str) -> Result<&str> {
        self.attrs
            .get(key)
            .map(String::as_str)
            .context("missing Qubes XML attribute")
    }

    fn value(&self) -> Value {
        let mut children: Vec<_> = self.children.iter().map(Node::value).collect();
        children.sort_by_cached_key(Value::to_string);
        json!([
            self.tag,
            self.attrs.iter().collect::<Vec<_>>(),
            self.text.trim(),
            children
        ])
    }
}

fn xml(path: &Path) -> Result<Node> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(state::LIMIT + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= state::LIMIT,
        "Qubes XML exceeds size limit"
    );
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0;
    loop {
        let event = reader.read_event()?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                count += 1;
                ensure!(
                    count <= 100000 && stack.len() < 128,
                    "Qubes XML complexity exceeds limit"
                );
                let mut attrs = BTreeMap::new();
                for attr in e.attributes() {
                    let attr = attr?;
                    ensure!(
                        attrs
                            .insert(
                                String::from_utf8(attr.key.as_ref().to_vec())?,
                                attr.decoded_and_normalized_value(
                                    quick_xml::XmlVersion::Implicit1_0,
                                    reader.decoder()
                                )?
                                .into_owned()
                            )
                            .is_none(),
                        "duplicate XML attribute"
                    );
                }
                let node = Node {
                    tag: String::from_utf8(e.name().as_ref().to_vec())?,
                    attrs,
                    text: String::new(),
                    children: Vec::new(),
                };
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        ensure!(root.replace(node).is_none(), "multiple XML roots");
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::Text(e) => {
                if let Some(node) = stack.last_mut() {
                    node.text
                        .push_str(&quick_xml::escape::unescape(&e.decode()?)?);
                } else {
                    ensure!(e.decode()?.trim().is_empty(), "unexpected XML text");
                }
            }
            Event::GeneralRef(e) => {
                let node = stack.last_mut().context("unexpected XML entity")?;
                if let Some(c) = e.resolve_char_ref()? {
                    node.text.push(c);
                } else {
                    let name = e.decode()?;
                    node.text.push_str(
                        quick_xml::escape::resolve_predefined_entity(&name)
                            .context("unsupported XML entity")?,
                    );
                }
            }
            Event::CData(e) => {
                stack
                    .last_mut()
                    .context("unexpected XML data")?
                    .text
                    .push_str(&e.decode()?);
            }
            Event::End(_) => {
                let node = stack.pop().context("unbalanced Qubes XML")?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    ensure!(root.replace(node).is_none(), "multiple XML roots");
                }
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
            _ => bail!("unsupported Qubes XML content"),
        }
    }
    ensure!(stack.is_empty(), "truncated Qubes XML");
    root.context("missing Qubes XML root")
}

fn metadata(path: &Path) -> Result<Value> {
    let m = fs::metadata(path)?;
    ensure!(
        m.is_file(),
        "artifact is not a regular file: {}",
        path.display()
    );
    Ok(json!([
        m.dev(),
        m.ino(),
        m.size(),
        m.mtime() as i128 * 1000000000 + m.mtime_nsec() as i128,
        m.ctime() as i128 * 1000000000 + m.ctime_nsec() as i128,
        m.mode()
    ]))
}

impl Coordinator {
    pub(super) fn boot_snapshot(
        &self,
        topology: &Topology,
        names: &[GuestName],
    ) -> Result<(LockHash, BTreeMap<GuestName, LockHash>)> {
        let root = xml(&configured_path(
            "AIRLOCK_QUBES_XML",
            "/var/lib/qubes/qubes.xml",
        )?)?;
        let pools: BTreeMap<_, _> = root
            .child("pools")?
            .children
            .iter()
            .map(|p| Ok((p.attr("name")?.to_owned(), p.attrs.clone())))
            .collect::<Result<_>>()?;
        let mut domains = BTreeMap::new();
        for d in &root.child("domains")?.children {
            let name = &d
                .child("properties")?
                .children
                .iter()
                .find(|p| p.attrs.get("name").is_some_and(|s| s == "name"))
                .context("missing VM name")?
                .text;
            ensure!(
                domains.insert(name.as_str(), d).is_none(),
                "duplicate VM name"
            );
        }
        let managed: BTreeSet<_> = names.iter().map(GuestName::as_str).collect();
        let mut owners = BTreeMap::new();
        for (name, d) in &domains {
            if let Ok(volumes) = d.child("volume-config") {
                for v in &volumes.children {
                    if !["root", "private"].contains(&v.attr("name")?) {
                        continue;
                    }
                    ensure!(
                        !managed.contains(name)
                            || v.attrs.get("source").is_none_or(String::is_empty),
                        "{name}: persistent volume has a foreign source"
                    );
                    let key = (v.attr("pool")?.to_owned(), v.attr("vid")?.to_owned());
                    if let Some(previous) = owners.insert(key, name.to_string()) {
                        ensure!(
                            !managed.contains(name) && !managed.contains(previous.as_str()),
                            "{name}: persistent disk shared with {previous}"
                        );
                    }
                }
            }
        }
        for (name, d) in &domains {
            if let Ok(volumes) = d.child("volume-config") {
                for v in &volumes.children {
                    for field in ["vid", "source"] {
                        if let (Some(pool), Some(id)) = (v.attrs.get("pool"), v.attrs.get(field))
                            && let Some(owner) = owners.get(&(pool.clone(), id.clone()))
                        {
                            ensure!(
                                !managed.contains(owner.as_str()) || owner == name,
                                "{name}: volume references {owner}'s persistent disk"
                            );
                        }
                    }
                }
            }
        }
        let policy = configured_path(
            "AIRLOCK_QUBES_POLICY_DIR",
            "/var/lib/airlock/qubes-policy.d",
        )?
        .join(
            std::env::var("AIRLOCK_QUBES_MANAGED_POLICY_FILE")
                .unwrap_or_else(|_| "90-airlock.policy".into()),
        );
        let mut policy_bytes = Vec::new();
        fs::File::open(policy)?
            .take(state::LIMIT + 1)
            .read_to_end(&mut policy_bytes)?;
        ensure!(
            policy_bytes.len() as u64 <= state::LIMIT,
            "policy exceeds size limit"
        );
        let owners: Vec<_> = owners.iter().map(|((p, v), n)| json!([p, v, n])).collect();
        let shared = state::digest(&json!({"pools":pools,"labels":root.child("labels")?.value(),"prefs":root.child("properties")?.value(),"owners":owners,"policy":format!("{:x}",Sha256::digest(policy_bytes))}))?.parse()?;
        let mut snapshots = BTreeMap::new();
        for name in names {
            let mut node = (*domains
                .get(name.as_str())
                .context("startup VM missing; use recovery")?)
            .clone();
            if let Ok(tags) = node.child("tags") {
                ensure!(
                    !tags
                        .children
                        .iter()
                        .any(|n| n.attrs.get("name").is_some_and(|s| s == "airlock-retired")),
                    "startup VM is retired: {name}"
                );
            }
            node.children.retain(|n| n.tag != "tags");
            if let Some(features) = node.children.iter_mut().find(|n| n.tag == "features") {
                features.children.retain(|n| {
                    n.attrs.get("name").is_some_and(|key| {
                        topology.domains[name].features.contains_key(key)
                            || key.starts_with("airlock-")
                            || key.starts_with("service.")
                    })
                });
            }
            let volumes = node
                .children
                .iter_mut()
                .find(|n| n.tag == "volume-config")
                .context("missing persistent volumes; use recovery")?;
            let kinds: BTreeSet<_> = volumes
                .children
                .iter()
                .map(|v| v.attr("name"))
                .collect::<Result<_>>()?;
            ensure!(
                ["root", "private", "store", "store-data"]
                    .iter()
                    .all(|n| kinds.contains(n)),
                "{name}: persistent or closure volumes missing"
            );
            let mut artifacts = BTreeMap::new();
            let mut disks = BTreeMap::new();
            volumes
                .children
                .retain(|v| v.attrs.get("name").is_none_or(|n| n != "volatile"));
            for volume in &volumes.children {
                let pool = pools.get(volume.attr("pool")?).context("missing pool")?;
                let kind = volume.attr("name")?;
                if ["root", "private"].contains(&kind)
                    && pool.get("driver").is_some_and(|s| s == "file")
                {
                    let path = Path::new(pool.get("dir_path").context("missing pool directory")?)
                        .join(format!("{}.img", volume.attr("vid")?));
                    let m = fs::metadata(&path)?;
                    ensure!(
                        m.is_file() && m.len() >= volume.attr("size")?.parse::<u64>()?,
                        "{name}: {kind} disk missing or truncated; use recovery"
                    );
                    disks.insert(path, m.len());
                }
                for field in ["view_path", "blob_map_path", "manifest_path"] {
                    if let Some(path) = volume.attrs.get(field).filter(|s| !s.is_empty()) {
                        let path = PathBuf::from(path);
                        artifacts.insert(path.clone(), metadata(&path)?);
                        if field == "manifest_path" {
                            for p in self.store().retained_root_files(
                                &path,
                                pool.get("root_dir").map(String::as_str),
                            )? {
                                artifacts.insert(p.clone(), metadata(&p)?);
                            }
                        }
                    }
                }
                if kind == "kernel"
                    && let Some(id) = volume.attrs.get("vid").filter(|s| !s.is_empty())
                {
                    let directory = Path::new(
                        pool.get("dir_path")
                            .map_or("/var/lib/qubes/vm-kernels", String::as_str),
                    )
                    .join(id);
                    ensure!(
                        directory.join("vmlinuz").is_file(),
                        "{name}: VM kernel missing; use recovery"
                    );
                    for entry in fs::read_dir(directory)? {
                        let p = entry?.path();
                        if p.is_file() {
                            artifacts.insert(p.clone(), metadata(&p)?);
                        }
                    }
                }
            }
            snapshots.insert(
                name.clone(),
                state::digest(&json!({"xml":node.value(),"artifacts":artifacts,"disks":disks}))?
                    .parse()?,
            );
        }
        Ok((shared, snapshots))
    }
}
