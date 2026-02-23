// OBJ.ASM → C conversion
// 3D object (alien) system — the core of Star Fox's entity management

#include "obj.h"
#include "boot.h"
#include "../sf_rtl.h"
#include <string.h>
#include <stdio.h>

Alien g_aliens[NUMBER_AL];
Alien g_view_block;

// Linked lists
static Alien *s_free_list = NULL;
static Alien *s_active_list = NULL;

void Obj_Init(void) {
    memset(g_aliens, 0, sizeof(g_aliens));
    memset(&g_view_block, 0, sizeof(g_view_block));

    // Build free list
    s_free_list = &g_aliens[0];
    s_active_list = NULL;

    for (int i = 0; i < NUMBER_AL - 1; i++) {
        g_aliens[i].next = &g_aliens[i + 1];
        g_aliens[i].prev = (i > 0) ? &g_aliens[i - 1] : NULL;
        g_aliens[i].active = false;
    }
    g_aliens[NUMBER_AL - 1].next = NULL;
    g_aliens[NUMBER_AL - 1].prev = &g_aliens[NUMBER_AL - 2];
    g_aliens[NUMBER_AL - 1].active = false;

    printf("Obj_Init: %d alien slots ready\n", NUMBER_AL);
}

Alien *Obj_Alloc(void) {
    if (!s_free_list) return NULL;

    Alien *al = s_free_list;
    s_free_list = al->next;

    // Clear alien data
    void *strategy_save = (void *)al->strategy;
    memset(al, 0, sizeof(Alien));
    al->active = true;
    (void)strategy_save;

    // Add to active list
    al->next = s_active_list;
    al->prev = NULL;
    if (s_active_list) s_active_list->prev = al;
    s_active_list = al;

    return al;
}

void Obj_Free(Alien *al) {
    if (!al || !al->active) return;

    // Remove from active list
    if (al->prev) al->prev->next = al->next;
    else s_active_list = al->next;
    if (al->next) al->next->prev = al->prev;

    // Add to free list
    al->active = false;
    al->next = s_free_list;
    al->prev = NULL;
    s_free_list = al;
}

Alien *Obj_GetByIndex(int idx) {
    if (idx < 0 || idx >= NUMBER_AL) return NULL;
    return &g_aliens[idx];
}

Alien *Obj_GetPlayer(void) {
    return &g_aliens[0];
}

void Obj_UpdateAll(void) {
    // Process all active aliens through their strategy functions
    Alien *al = s_active_list;
    while (al) {
        Alien *next = al->next;  // Save next in case strategy frees this alien
        if (al->active && al->strategy) {
            al->strategy(al);
        }
        al = next;
    }
}
