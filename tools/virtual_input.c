// virtual_input: inject pointer motion, clicks and scroll into a wlroots compositor
// (cage) through the wlr virtual-pointer protocol, for the headless runtime-capture
// workflow (.github/workflows/launcher-runtime-shot.yml). The compositor forwards the
// events to the focused client as ordinary pointer input, so the app under test sees a
// real pointer. Build (see the workflow):
//   wayland-scanner client-header tools/wlr-virtual-pointer-unstable-v1.xml vp.h
//   wayland-scanner private-code  tools/wlr-virtual-pointer-unstable-v1.xml vp.c
//   gcc -O2 -Wall -o virtual-input tools/virtual_input.c vp.c -I. $(pkg-config --cflags --libs wayland-client)
//
// Usage (all coordinates/extents unsigned integers, STEPS a signed integer):
//   virtual_input move   X Y EX EY
//   virtual_input click  X Y EX EY        (left button, at X,Y)
//   virtual_input scroll X Y EX EY STEPS  (positive STEPS scrolls down, |STEPS| <= 50)
// EX/EY are the current output extents for absolute motion. WAYLAND_DISPLAY must be set
// by the caller. Exit 0 only when every request round-tripped; compositor acceptance
// (exit 0) is not proof the app reacted - that evidence is the captured frames.
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "vp.h"

#define BTN_LEFT 0x110
#define MAX_COORD 100000ul
#define MAX_STEPS 50

static struct wl_seat *seat = NULL;
static struct zwlr_virtual_pointer_manager_v1 *vpm = NULL;

static void die(const char *msg) {
    fprintf(stderr, "virtual_input: %s\n", msg);
    exit(64);
}

static uint32_t parse_u32(const char *s, const char *name) {
    if (s == NULL || *s == '\0') die("missing numeric argument");
    char *end = NULL;
    errno = 0;
    unsigned long v = strtoul(s, &end, 10);
    if (errno != 0 || end == s || *end != '\0' || v > MAX_COORD) {
        fprintf(stderr, "virtual_input: bad %s '%s'\n", name, s);
        exit(64);
    }
    return (uint32_t)v;
}

static int parse_steps(const char *s) {
    if (s == NULL || *s == '\0') die("missing STEPS");
    char *end = NULL;
    errno = 0;
    long v = strtol(s, &end, 10);
    if (errno != 0 || end == s || *end != '\0' || v > MAX_STEPS || v < -MAX_STEPS || v == 0) {
        fprintf(stderr, "virtual_input: bad STEPS '%s' (want -%d..%d, nonzero)\n", s, MAX_STEPS, MAX_STEPS);
        exit(64);
    }
    return (int)v;
}

static void registry_global(void *data, struct wl_registry *reg, uint32_t name,
                            const char *interface, uint32_t version) {
    (void)data; (void)version;
    if (strcmp(interface, wl_seat_interface.name) == 0 && seat == NULL)
        seat = wl_registry_bind(reg, name, &wl_seat_interface, 1);
    else if (strcmp(interface, zwlr_virtual_pointer_manager_v1_interface.name) == 0)
        vpm = wl_registry_bind(reg, name, &zwlr_virtual_pointer_manager_v1_interface, 1);
}
static void registry_remove(void *data, struct wl_registry *reg, uint32_t name) {
    (void)data; (void)reg; (void)name;
}

static uint32_t now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)(ts.tv_sec * 1000u + ts.tv_nsec / 1000000u);
}

static void roundtrip(struct wl_display *dpy, const char *what) {
    if (wl_display_roundtrip(dpy) < 0) {
        fprintf(stderr, "virtual_input: wayland roundtrip failed at %s (display error or disconnect)\n", what);
        exit(4);
    }
}

int main(int argc, char **argv) {
    if (argc < 2) die("usage: move|click X Y EX EY | scroll X Y EX EY STEPS");
    enum { VERB_NONE, VERB_MOVE, VERB_CLICK, VERB_SCROLL } verb = VERB_NONE;
    if (strcmp(argv[1], "move") == 0) verb = VERB_MOVE;
    else if (strcmp(argv[1], "click") == 0) verb = VERB_CLICK;
    else if (strcmp(argv[1], "scroll") == 0) verb = VERB_SCROLL;
    else die("unknown verb (want move, click or scroll)");
    int need = (verb == VERB_SCROLL) ? 7 : 6;
    if (argc != need) die(verb == VERB_SCROLL ? "usage: scroll X Y EX EY STEPS"
                                              : "usage: move|click X Y EX EY");

    uint32_t x = parse_u32(argv[2], "X");
    uint32_t y = parse_u32(argv[3], "Y");
    uint32_t ex = parse_u32(argv[4], "EX");
    uint32_t ey = parse_u32(argv[5], "EY");
    if (ex == 0 || ey == 0) die("EX/EY must be nonzero");
    if (x > ex || y > ey) die("X/Y outside the given extents");
    int steps = (verb == VERB_SCROLL) ? parse_steps(argv[6]) : 0;

    if (getenv("WAYLAND_DISPLAY") == NULL)
        die("WAYLAND_DISPLAY is not set; the caller must pass the compositor socket name");
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) die("cannot connect to the wayland display");
    struct wl_registry *reg = wl_display_get_registry(dpy);
    static const struct wl_registry_listener listener = { registry_global, registry_remove };
    wl_registry_add_listener(reg, &listener, NULL);
    roundtrip(dpy, "registry");
    if (seat == NULL || vpm == NULL) {
        fprintf(stderr, "virtual_input: compositor lacks wl_seat (%p) or virtual-pointer manager (%p)\n",
                (void *)seat, (void *)vpm);
        return 3;
    }
    struct zwlr_virtual_pointer_v1 *vp = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(vpm, seat);
    roundtrip(dpy, "create virtual pointer");

    zwlr_virtual_pointer_v1_motion_absolute(vp, now_ms(), x, y, ex, ey);
    zwlr_virtual_pointer_v1_frame(vp);
    roundtrip(dpy, "motion");
    usleep(100000);

    if (verb == VERB_CLICK) {
        zwlr_virtual_pointer_v1_button(vp, now_ms(), BTN_LEFT, 1);
        zwlr_virtual_pointer_v1_frame(vp);
        roundtrip(dpy, "button press");
        usleep(90000);
        zwlr_virtual_pointer_v1_button(vp, now_ms(), BTN_LEFT, 0);
        zwlr_virtual_pointer_v1_frame(vp);
        roundtrip(dpy, "button release");
    } else if (verb == VERB_SCROLL) {
        int dir = steps < 0 ? -1 : 1;
        if (steps < 0) steps = -steps;
        for (int i = 0; i < steps; i++) {
            zwlr_virtual_pointer_v1_axis(vp, now_ms(), 0 /* vertical */,
                                         wl_fixed_from_double(15.0 * dir));
            zwlr_virtual_pointer_v1_frame(vp);
            roundtrip(dpy, "axis");
            usleep(70000);
        }
    }

    zwlr_virtual_pointer_v1_destroy(vp);
    roundtrip(dpy, "destroy");
    wl_display_disconnect(dpy);
    return 0;
}
