#ifndef STARFOX_VIEWER_SHAPE_CATALOG_ASM_H
#define STARFOX_VIEWER_SHAPE_CATALOG_ASM_H

#include "../types.h"
#include <stdbool.h>

typedef enum {
    VIEWER_PALETTE_NORM = 0,
    VIEWER_PALETTE_RED = 1,
    VIEWER_PALETTE_BLUE = 2,
    VIEWER_PALETTE_COUNT = 3,
} ViewerPaletteKind;

typedef struct {
    const char *label;
    const char *display_name;
    const char *points_label;
    const char *faces_label;
    const char *color_table_label;
    int shift;
    int frame_count;
    int color_frame_count;
    int vertex_count;
    int poly_face_count;
    int line_face_count;
    bool has_textured_materials;
} ViewerShapeInfo;

typedef struct {
    const char *file_path;
    const char *line_text;
    const char *reason;
    int line_number;
} ViewerSkippedShapeInfo;

bool ViewerCatalog_LoadFromAsm(void);
void ViewerCatalog_Unload(void);

int ViewerCatalog_GetShapeHeaderCount(void);
int ViewerCatalog_GetShapeCount(void);
const ViewerShapeInfo *ViewerCatalog_GetShapeInfo(int index);
int ViewerCatalog_FindShapeByLabel(const char *label);
int ViewerCatalog_GetShapeFrameCount(int index);
int ViewerCatalog_GetShapeColorFrameCount(int index);
bool ViewerCatalog_GetShapeBounds(int index, float *out_center_xyz, float *out_radius);
int ViewerCatalog_GetSkippedShapeCount(void);
const ViewerSkippedShapeInfo *ViewerCatalog_GetSkippedShapeInfo(int index);

void ViewerCatalog_SetPalette(ViewerPaletteKind palette);
ViewerPaletteKind ViewerCatalog_GetPalette(void);
void ViewerCatalog_NextPalette(int delta);
const char *ViewerCatalog_GetPaletteName(ViewerPaletteKind palette);
void ViewerCatalog_SetShadeTable(int shade_table);
int ViewerCatalog_GetShadeTable(void);
void ViewerCatalog_NextShadeTable(int delta);

void ViewerCatalog_RenderShape(int index, int anim_frame, int col_frame,
                               const float *model_matrix,
                               const float *view_matrix,
                               const float *proj_matrix);

#endif
