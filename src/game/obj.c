// OBJ.ASM → C conversion
// 3D object (alien) system — the core of Star Fox's entity management.
// Decompiled from OBJ.ASM, MAIN.ASM (kill_list_l, newal_l, removedeadal_l),
// and TRANS.ASM (dostrats strategy loop).
//
// Every 3D entity (enemies, player, items, obstacles) is an Alien struct
// managed in doubly-linked lists: an active list (allst) and a free list
// (alfreelst). Each frame, dostrats walks the active list and calls each
// alien's strategy function.

#include "obj.h"
#include "game_vars.h"
#include "boot.h"
#include "../map/map_exec.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <string.h>
#include <stdio.h>

Alien g_aliens[NUMBER_AL];
Alien g_view_block;

// Linked list heads (allst / alfreelst in ASM)
Alien *g_active_list = NULL;
Alien *g_free_list = NULL;

// Death flag: strategies set this to request removal of the current alien.
// Checked after each do_strat_l call in the dostrats loop.
uint8 g_aldead = 0;

// --- Linked list helpers (matching SNES doubly-linked list ops) ---

// Unlink an alien from whatever list it's in.
// Updates the given head pointer if the alien was the head.
static void list_unlink(Alien **head, Alien *al) {
    if (al->prev) al->prev->next = al->next;
    else *head = al->next;
    if (al->next) al->next->prev = al->prev;
    al->next = NULL;
    al->prev = NULL;
}

// Insert alien at the head of a list.
static void list_push_front(Alien **head, Alien *al) {
    al->next = *head;
    al->prev = NULL;
    if (*head) (*head)->prev = al;
    *head = al;
}

// --- Public API ---

void Obj_Init(void) {
    // From kill_list_l (MAIN.ASM:1992) + FmtFreeLst macro:
    // Clear all alien data, build free list, empty active list.
    memset(g_aliens, 0, sizeof(g_aliens));
    memset(&g_view_block, 0, sizeof(g_view_block));

    g_active_list = NULL;
    g_free_list = NULL;

    // Build free list as doubly-linked chain of all alien slots.
    // FmtFreeLst alfreelst,alblks,number_al,al_size
    for (int i = NUMBER_AL - 1; i >= 0; i--) {
        g_aliens[i].active = false;
        list_push_front(&g_free_list, &g_aliens[i]);
    }

    g_aldead = 0;
    printf("Obj_Init: %d alien slots ready\n", NUMBER_AL);
}

void Obj_KillAll(void) {
    // kill_list_l (MAIN.ASM:1992): clear active list, rebuild free list
    // Called on level transitions.
    g_active_list = NULL;
    g_free_list = NULL;

    for (int i = 0; i < NUMBER_AL; i++) {
        Alien *al = &g_aliens[i];
        // Preserve nothing — full wipe
        memset(al, 0, sizeof(Alien));
        al->active = false;
        list_push_front(&g_free_list, al);
    }
}

Alien *Obj_Alloc(void) {
    // newal_l / getnewal_l: take from free list, insert into active list.
    if (!g_free_list) return NULL;

    Alien *al = g_free_list;
    list_unlink(&g_free_list, al);

    // Zero the alien data (SNES clears the block on allocation)
    StrategyFunc saved_strat = NULL;  // will be set by caller
    (void)saved_strat;
    memset(al, 0, sizeof(Alien));
    al->active = true;
    al->collflags = ACF_FIRSTFRAME;  // First frame flag (cleared after first strategy tick)

    // Insert at head of active list
    list_push_front(&g_active_list, al);

    return al;
}

void Obj_Free(Alien *al) {
    // removedeadal_l: unlink from active list, return to free list.
    if (!al || !al->active) return;

    list_unlink(&g_active_list, al);

    al->active = false;
    al->stratptr = NULL;

    list_push_front(&g_free_list, al);
}

Alien *Obj_GetByIndex(int idx) {
    if (idx < 0 || idx >= NUMBER_AL) return NULL;
    return &g_aliens[idx];
}

Alien *Obj_GetPlayer(void) {
    // Player is always the first alien allocated (g_aliens[0] by convention).
    // Walk active list to find it — or just return g_aliens[0] if active.
    if (g_aliens[0].active) return &g_aliens[0];
    return NULL;
}

// init_strats_l (partial literal port, GSTRATS.ASM:423-434)
// This runs once per strategy frame before update_objects_l.
static void init_strats_l(void) {
    Alien *player = NULL;

    // GSTRATS.ASM uses internalPLAYPT as the authoritative player pointer.
    if (g_internalPLAYPT >= 0 && g_internalPLAYPT < NUMBER_AL) {
        Alien *candidate = &g_aliens[g_internalPLAYPT];
        if (candidate->active) {
            player = candidate;
        }
    }

    // Fallback for early boot or incomplete pointer setup.
    if (!player) {
        player = Obj_GetPlayer();
    }
    if (!player || !player->active) {
        return;
    }

    // a16:
    //   lda al_worldx,x -> player_posx
    //   lda al_worldy,x -> player_posy
    //   lda al_worldz,x -> player_posz
    g_player_posx = player->worldx;
    g_player_posy = player->worldy;
    g_player_posz = player->worldz;

    // s_copy_alvar2var W,x,playervelZ,al_vz
    g_playervelZ = player->vz;
}

// do_strat_l (partial literal port, STRATROU.ASM:2189+)
// Dispatch order matters and matches the ASM:
// 1) dead-object end-collision/exp callbacks
// 2) clear nuked type bit
// 3) collision callback path
// 4) L-collision callback path
// 5) normal strategy callback
static void do_strat_l(Alien *al) {
    if (!al || !al->active) return;

    // STRATROU.ASM: cpx dummyobj / lbeq .estrat
    if (g_dummyobj > 0) {
        int idx = (int)(al - g_aliens);
        if (idx == g_dummyobj) {
            return;
        }
    }

    // STRATROU.ASM: s_clr_alcollflag x,firstframe
    al->collflags &= ~ACF_FIRSTFRAME;

    // Dead object path (unless nuked)
    if (al->HP == 0 && !(al->type & ATNUKED)) {
        if (al->endcollstratptr && (al->sflags & ASF_LCOLLIDE)) {
            al->sflags &= ~ASF_LCOLLIDE;
            al->endcollstratptr(al);
            return;
        }
        if (al->expstratptr) {
            al->expstratptr(al);
            return;
        }
    }

    // STRATROU.ASM: s_clr_altype x,nuked
    al->type &= ~ATNUKED;

    if (al->sflags & ASF_COLLIDE) {
        if (al->collstratptr) {
            al->collstratptr(al);
        } else {
            al->sflags &= ~ASF_COLLIDE;
        }
        return;
    }

    if (al->sflags & ASF_LCOLLIDE) {
        al->sflags &= ~ASF_LCOLLIDE;
        if (al->endcollstratptr) {
            al->endcollstratptr(al);
        }
        return;
    }

    if (al->stratptr) {
        al->stratptr(al);
    }
}

void Obj_RunStrategies(void) {
    // Decompiled from dostrats (TRANS.ASM:312-334).
    // This is the heart of the game — called once per frame.
    //
    // dostrats:
    //   incw gameframe
    //   jsl  init_strats_l       ; per-frame init
    //   jsl  update_objects_l    ; map spawning based on player Z
    //   ldx  allst               ; walk active list
    // stratlp:
    //   stz  aldead              ; clear death flag
    //   jsl  do_strat_l          ; call this alien's strategy
    //   lda  aldead
    //   bne  .killal             ; if marked dead, remove
    //   ldy  _next,x             ; advance to next alien
    // .killed: tyx
    //   bne  stratlp
    //   bra  .cont
    // .killal: ldy _next,x
    //   jsl  removedeadal_l
    //   bra  .killed

    // Step 1: Increment game frame counter
    g_gameframe++;

    // Step 2: Per-frame init (init_strats_l)
    init_strats_l();

    // Step 3: Update map VM (update_objects_l from WORLD.ASM)
    // Tracks player Z position changes and triggers map-script spawns.
    MapExec_Tick();

    // Step 4: Strategy loop — walk active list, call each alien's strategy
    Alien *al = g_active_list;
    while (al) {
        g_aldead = 0;  // stz aldead

        do_strat_l(al);

        Alien *next_al = al->next;  // Save _next before possible removal

        if (g_aldead) {
            // removedeadal_l: remove dead alien from active list
            Obj_Free(al);
        }

        al = next_al;
    }
}
