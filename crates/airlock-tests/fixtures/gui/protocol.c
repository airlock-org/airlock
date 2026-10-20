/* Execute the actual Qubes window protocol implementation carried by the patches. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef unsigned long Window;
typedef struct { int type; void *display; Window window; unsigned long message_type;
    int format; union { long l[5]; } data; } XClientMessageEvent;
typedef XClientMessageEvent XEvent;
typedef struct { void *display, *vchan; Window root_win;
    unsigned long wm_moveresize, wm_state_maximized_vert, wm_state_maximized_horz, wm_change_state;
    uint32_t protocol_version; } Ghandles;
struct windowdata { int is_mapped, override_redirect; Window local_winid; };
struct msg_hdr { uint32_t window, type; };
struct msg_window_flags { uint32_t flags_set, flags_unset; };
#define MSG_WINDOW_FLAGS 1
#define MSG_WINDOW_MOVERESIZE 2
#define QUBES_GUID_MIN_WINDOW_MOVERESIZE 0x00010009
#define IconicState 3
#define False 0
#define ClientMessage 33
#define SubstructureNotifyMask 1
#define SubstructureRedirectMask 2
#define Button1Mask 256
static uint32_t direction, button;
static int pointer_ok = 1;
static unsigned int buttons;
static XClientMessageEvent events[8];
static int count, known_window = 1;
static void *windows_list;
static int lookup_window(Ghandles *g, void *list, Window window, const char *name) { return known_window; }
#define write_message(channel, hdr, msg) do { uint32_t values[2]; memcpy(values, &(msg), sizeof(values)); \
    events[count].data.l[0]=values[0]; events[count].data.l[1]=values[1]; \
    events[count].data.l[2]=(hdr).type; count++; } while (0)
#define read_struct(channel, msg) do { (msg).direction = direction; (msg).button = button; } while (0)
static int XQueryPointer(void *display, Window root, Window *rr, Window *cr,
    int *rx, int *ry, int *wx, int *wy, unsigned int *mask) {
    *rx = 120; *ry = 240; *mask = buttons; return pointer_ok;
}
static void XSendEvent(void *display, Window root, int propagate, long mask, XEvent *event) {
    events[count++] = *event;
}

@flags@
@wire@
@move@
static void maximize(Ghandles *g, struct msg_window_flags msg) {
XClientMessageEvent ev = {0}; uint32_t flags_all = msg.flags_set | msg.flags_unset;
@maximize@
}
@agent@

int main(int argc, char **argv) {
    Ghandles g = {.root_win=1, .wm_moveresize=2, .wm_state_maximized_vert=3,
        .wm_state_maximized_horz=4};
    if (!strcmp(argv[1], "maximize")) {
        struct msg_window_flags msg = {.flags_set=strtoul(argv[2],0,0), .flags_unset=strtoul(argv[3],0,0)};
        maximize(&g, msg);
    } else if (!strcmp(argv[1], "agent")) {
        g.wm_change_state=5; g.protocol_version=strtoul(argv[2],0,0); known_window=atoi(argv[3]);
        XClientMessageEvent ev={.message_type=atoi(argv[4]), .format=atoi(argv[5]), .window=7};
        ev.data.l[0]=3; ev.data.l[2]=8; ev.data.l[3]=1; forward(&g, &ev);
    } else {
        direction=atoi(argv[2]); button=atoi(argv[3]); buttons=atoi(argv[4]);
        struct windowdata window = {.is_mapped=atoi(argv[5]), .override_redirect=atoi(argv[6]), .local_winid=7};
        pointer_ok=atoi(argv[7]); handle_window_moveresize(&g, &window);
    }
    printf("[");
    for (int i=0;i<count;i++) {
        if(i) printf(",");
        printf("[%ld,%ld,%ld,%ld]",events[i].data.l[0],events[i].data.l[1],events[i].data.l[2],events[i].data.l[3]);
    }
    printf("]\n"); return 0;
}
