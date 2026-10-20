/* Rebinding actual patched Xorg buffers must preserve addresses and application contents. */
#define _GNU_SOURCE
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
typedef int Bool;
#define TRUE 1
#define FALSE 0
#define XC_PAGE_SIZE 4096
#define X_INFO 0
struct xf86_qubes_pixmap {
    struct xf86_qubes_pixmap *next;
    size_t pages;
    uint32_t *refs;
    uint8_t *data;
    Bool shared;
};
struct driver { void *xgs; uint32_t gui_domid; int scrnIndex; } driver;
typedef struct driver *DUMMYPtr;
#define DUMMYScrn (&driver)
#define DUMMYPTR(x) (x)
static struct xf86_qubes_pixmap *all_pixmaps;
static Bool gui_detached;
static Bool fail_allocation;
static uint32_t granted_to;
#define xf86DrvMsg(...) ((void)0)
static void xf86_qubes_pixmap_remove_list_all(void) {}
static void *xengntshr_share_pages(void *handle, uint32_t domid, size_t pages,
                                 uint32_t *refs, int writable) {
    (void)handle; (void)writable;
    if (fail_allocation) return NULL;
    granted_to = domid;
    for (size_t i = 0; i < pages; i++) refs[i] = domid + i;
    return mmap(NULL, pages * XC_PAGE_SIZE, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
}
@function@
int main(void) {
    uint32_t refs[2] = {1, 2};
    struct xf86_qubes_pixmap priv = {.pages=2, .refs=refs, .shared=TRUE};
    priv.data = mmap(NULL, 8192, PROT_READ | PROT_WRITE,
                     MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    assert(priv.data != MAP_FAILED);
    memset(priv.data, 42, 8192);
    uint8_t *original = priv.data;
    all_pixmaps = &priv;
    assert(xf86_qubes_reconnect(10, FALSE));
    assert(priv.data == original && gui_detached && !priv.shared);
    for (size_t i = 0; i < 8192; i++) assert(priv.data[i] == 42);
    memset(priv.data, 43, 8192); /* application keeps drawing while detached */
    fail_allocation = TRUE;
    assert(!xf86_qubes_reconnect(99, TRUE));
    assert(priv.data == original && priv.data[8191] == 43);
    fail_allocation = FALSE;
    assert(xf86_qubes_reconnect(99, TRUE));
    assert(!gui_detached && priv.shared && granted_to == 99);
    assert(priv.data == original && priv.refs[0] == 99 && priv.refs[1] == 100);
    for (size_t i = 0; i < 8192; i++) assert(priv.data[i] == 43);
    assert(!xf86_qubes_reconnect(65536, TRUE));
    munmap(priv.data, 8192);
    return 0;
}
