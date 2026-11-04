// virtual_input: inject pointer motion, clicks and scroll into a wlroots compositor
// (cage) through the wlr virtual-pointer protocol, for the headless runtime-capture
// workflow (.github/workflows/launcher-runtime-shot.yml). The compositor forwards the
// events to the focused client as ordinary pointer input, so the app under test sees a
// real pointer. Build (see the workflow):
//   wayland-scanner client-header tools/wlr-virtual-pointer-unstable-v1.xml vp.h
//   wayland-scanner private-code  tools/wlr-virtual-pointer-unstable-v1.xml vp.c
//   gcc -O2 -Wall -o virtual-input tools/virtual_input.c vp.c -I. $(pkg-config --cflags --libs wayland-client)
//
// Usage:
//   virtual_input move   X Y EX EY
//   virtual_input click  X Y EX EY        (left button, at X,Y)
//   virtual_input scroll X Y EX EY STEPS  (positive STEPS scrolls down)
// EX/EY are the current output extents for absolute motion.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "vp.h"

#define BTN_LEFT 0x110

static struct wl_seat *seat = NULL;
static struct zwlr_virtual_pointer_manager_v1 *vpm = NULL;

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

static void move_to(struct zwlr_virtual_pointer_v1 *vp, uint32_t x, uint32_t y,
                    uint32_t ex, uint32_t ey) {
    zwlr_virtual_pointer_v1_motion_absolute(vp, now_ms(), x, y, ex, ey);
    zwlr_virtual_pointer_v1_frame(vp);
}

int main(int argc, char **argv) {
    if (argc >= 2 && (strcmp(argv[1], "scroll") == 0) && argc != 7) {
        fprintf(stderr, "usage: scroll X Y EX EY STEPS\n");
        return 64;
    }
    if (argc >= 2 && strcmp(argv[1], "scroll") != 0 && argc != 6) {
        fprintf(stderr, "usage: move|click X Y EX EY\n");
        return 64;
    }
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) { fprintf(stderr, "virtual_input: cannot connect to wayland display\n"); return 2; }
    struct wl_registry *reg = wl_display_get_registry(dpy);
    static const struct wl_registry_listener listener = { registry_global, registry_remove };
    wl_registry_add_listener(reg, &listener, NULL);
    wl_display_roundtrip(dpy);
    if (seat == NULL || vpm == NULL) {
        fprintf(stderr, "virtual_input: compositor lacks wl_seat (%p) or virtual-pointer manager (%p)\n",
                (void *)seat, (void *)vpm);
        return 3;
    }
    struct zwlr_virtual_pointer_v1 *vp = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(vpm, seat);
    wl_display_roundtrip(dpy);

    uint32_t x = (uint32_t)strtoul(argv[2], NULL, 10);
    uint32_t y = (uint32_t)strtoul(argv[3], NULL, 10);
    uint32_t ex = (uint32_t)strtoul(argv[4], NULL, 10);
    uint32_t ey = (uint32_t)strtoul(argv[5], NULL, 10);

    move_to(vp, x, y, ex, ey);
    wl_display_roundtrip(dpy);
    usleep(100000);

    if (strcmp(argv[1], "click") == 0) {
        zwlr_virtual_pointer_v1_button(vp, now_ms(), BTN_LEFT, 1);
        zwlr_virtual_pointer_v1_frame(vp);
        wl_display_roundtrip(dpy);
        usleep(90000);
        zwlr_virtual_pointer_v1_button(vp, now_ms(), BTN_LEFT, 0);
        zwlr_virtual_pointer_v1_frame(vp);
        wl_display_roundtrip(dpy);
    } else if (strcmp(argv[1], "scroll") == 0) {
        int steps = atoi(argv[6]);
        int dir = steps < 0 ? -1 : 1;
        if (steps < 0) steps = -steps;
        for (int i = 0; i < steps; i++) {
            zwlr_virtual_pointer_v1_axis(vp, now_ms(), 0 /* vertical */,
                                         wl_fixed_from_double(15.0 * dir));
            zwlr_virtual_pointer_v1_frame(vp);
            wl_display_roundtrip(dpy);
            usleep(70000);
        }
    }

    zwlr_virtual_pointer_v1_destroy(vp);
    wl_display_roundtrip(dpy);
    wl_display_disconnect(dpy);
    return 0;
}
