// WINDOWS.ASM -> C conversion
// Color window/fade state only (no SNES register writes).

#include "windows.h"
#include "game_vars.h"
#include <string.h>

#define MSTARWIPE_CIRCLE 1

static int window_find_mode(uint8 mode) {
    for (int i = 0; i < WINDOWARRAY_SIZE; i++) {
        if ((g_windowmode & (1u << i)) != 0 && g_windowarray[i].mode == mode) {
            return i;
        }
    }
    return -1;
}

static int window_alloc(uint8 mode) {
    for (int i = 0; i < WINDOWARRAY_SIZE; i++) {
        if ((g_windowmode & (1u << i)) == 0) {
            g_windowmode |= (uint8)(1u << i);
            memset(&g_windowarray[i], 0, sizeof(g_windowarray[i]));
            g_windowarray[i].mode = mode;
            return i;
        }
    }
    return -1;
}

static int window_get_or_alloc(uint8 mode) {
    int slot = window_find_mode(mode);
    if (slot >= 0) return slot;
    return window_alloc(mode);
}

static void window_dealloc(int slot) {
    if (slot < 0 || slot >= WINDOWARRAY_SIZE) return;
    g_windowmode &= (uint8)~(1u << slot);
    memset(&g_windowarray[slot], 0, sizeof(g_windowarray[slot]));
}

static void update_setblack_slot(int slot) {
    WindowState *w = &g_windowarray[slot];
    uint8 test = (uint8)(w->stayblack + 1);
    if (test == 0) {
        window_dealloc(slot);
        return;
    }

    if (w->stayblack != 0) {
        w->stayblack = (uint8)(w->stayblack - 1);
        return;
    }

    if (g_oncewipe == 0) {
        g_oncewipe = 1;
        g_circleanim = MSTARWIPE_CIRCLE;
        w->stayblack = (uint8)(w->stayblack - 1);
        return;
    }

    if (w->wm_val != 0) {
        w->wm_val = (w->wm_val >= 2) ? (uint8)(w->wm_val - 2) : 0;
        return;
    }

    w->stayblack = (uint8)(w->stayblack - 1);
}

static void update_fadewhite_slot(int slot) {
    WindowState *w = &g_windowarray[slot];
    if (w->wm_val < 31) {
        w->wm_val++;
    }
}

static void update_fadewhite2norm_slot(int slot) {
    WindowState *w = &g_windowarray[slot];

    if (w->wm_val > 0) {
        w->wm_val--;
    }

    if (w->wm_val == 0) {
        window_dealloc(slot);
    }
}

static void update_mapfade_slot(int slot) {
    WindowState *w = &g_windowarray[slot];

    if (g_fadedir == 0) return;

    if (g_fadedir < 0) {
        uint8 step = (uint8)(-g_fadedir);
        uint16 next = (uint16)w->wm_val + (uint16)step;
        w->wm_val = (next >= 30) ? 30 : (uint8)next;
        if (w->wm_val >= 30) {
            g_fadedir = 0;
        }
        return;
    }

    uint8 step = (uint8)g_fadedir;
    if (w->wm_val <= step) {
        w->wm_val = 0;
    } else {
        w->wm_val = (uint8)(w->wm_val - step);
    }

    if (w->wm_val == 0) {
        g_fadedir = 0;
        window_dealloc(slot);
    }
}

void Windows_Init(void) {
    g_windowmode = 0;
    g_fadedir = 0;
    memset(g_windowarray, 0, sizeof(g_windowarray));
}

void Windows_Update(void) {
    for (int i = 0; i < WINDOWARRAY_SIZE; i++) {
        if ((g_windowmode & (1u << i)) == 0) continue;

        switch (g_windowarray[i].mode) {
        case WINDOW_MODE_BLACK:
            update_setblack_slot(i);
            break;
        case WINDOW_MODE_WHITEFADE:
            update_fadewhite_slot(i);
            break;
        case WINDOW_MODE_WHITE2NORM:
            update_fadewhite2norm_slot(i);
            break;
        case WINDOW_MODE_MAPFADE:
            update_mapfade_slot(i);
            break;
        default:
            break;
        }
    }
}

void Windows_StartMapFade(int8 fadedir) {
    if (fadedir == 0) {
        g_fadedir = 0;
        return;
    }

    if (fadedir > 2) fadedir = 2;
    if (fadedir < -2) fadedir = -2;

    int slot = window_get_or_alloc(WINDOW_MODE_MAPFADE);
    if (slot < 0) {
        g_fadedir = 0;
        return;
    }

    WindowState *w = &g_windowarray[slot];
    w->mode = WINDOW_MODE_MAPFADE;
    w->stayblack = 0;

    if (fadedir < 0) {
        if (w->wm_val > 30) w->wm_val = 30;
    } else {
        if (w->wm_val == 0) w->wm_val = 30;
    }

    g_fadedir = fadedir;
}

bool Windows_IsMapFadeActive(void) {
    return g_fadedir != 0;
}

void Windows_FadeToBlack(int speed) {
    int8 step = (speed >= 2) ? 2 : 1;
    Windows_StartMapFade((int8)-step);
}

void Windows_FadeFromBlack(int speed) {
    int8 step = (speed >= 2) ? 2 : 1;
    Windows_StartMapFade(step);
}

void Windows_FadeToWhite(int speed) {
    (void)speed;

    int slot = window_get_or_alloc(WINDOW_MODE_WHITEFADE);
    if (slot < 0) return;

    g_windowarray[slot].mode = WINDOW_MODE_WHITEFADE;
    g_windowarray[slot].stayblack = 0;
}

void initblack_l(void) {
    int slot = window_get_or_alloc(WINDOW_MODE_BLACK);
    if (slot < 0) return;

    g_windowarray[slot].mode = WINDOW_MODE_BLACK;
    g_windowarray[slot].stayblack = 40;
    g_windowarray[slot].wm_val = 30;
    setblack_l();
}

void setblack_l(void) {
    int slot = window_find_mode(WINDOW_MODE_BLACK);
    if (slot < 0) return;
    update_setblack_slot(slot);
}

void fadewhite(void) {
    int slot = window_get_or_alloc(WINDOW_MODE_WHITEFADE);
    if (slot < 0) return;
    update_fadewhite_slot(slot);
}

void initfadewhite2norm_l(void) {
    int slot = window_find_mode(WINDOW_MODE_WHITEFADE);
    if (slot < 0) slot = window_get_or_alloc(WINDOW_MODE_WHITE2NORM);
    if (slot < 0) return;

    g_windowarray[slot].mode = WINDOW_MODE_WHITE2NORM;
    g_windowarray[slot].stayblack = 0;
    g_windowarray[slot].wm_val = 31;
}

void fadewhite2norm(void) {
    int slot = window_find_mode(WINDOW_MODE_WHITE2NORM);
    if (slot < 0) return;
    update_fadewhite2norm_slot(slot);
}
