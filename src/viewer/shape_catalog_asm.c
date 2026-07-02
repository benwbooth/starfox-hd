#include "shape_catalog_asm.h"

#include "../renderer/gl_backend.h"
#include "../renderer/transform.h"

#include <glad/glad.h>

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define VIEWER_MAX_FACE_VERTS 12
#define VIEWER_DEBUG_MAGENTA_R 1.0f
#define VIEWER_DEBUG_MAGENTA_G 0.0f
#define VIEWER_DEBUG_MAGENTA_B 1.0f

#define MATERIAL_COLANIM(anim_id) ((uint16)(0x8000u | ((uint16)(anim_id) & 0x3FFFu)))
#define MATERIAL_COLTEXT(tex_id, tex_xy) ((uint16)(0x4000u | ((((uint16)(tex_xy)) & 0x3Fu) << 8) | (((uint16)(tex_id)) & 0xFFu)))
#define MATERIAL_COLLITE(light_source, normal_color) ((uint16)(((((uint16)(light_source)) & 0x3Fu) << 8) | (((uint16)(normal_color)) & 0xFFu)))
#define MATERIAL_COLDEPTH(index) ((uint16)((62u << 8) | (((uint16)(index)) & 0xFFu)))
#define MATERIAL_COLNORM(lo, hi) ((uint16)((63u << 8) | ((((uint16)(hi)) & 0x0Fu) << 4) | (((uint16)(lo)) & 0x0Fu)))
#define MATERIAL_COLSMOOTH(light_source, normal_color) ((uint16)(0xC000u | ((((uint16)(light_source)) & 0x3Fu) << 8) | (((uint16)(normal_color)) & 0xFFu)))

#define VIEWER_ARRAY_GROW(array, count, capacity, type) \
    do { \
        if ((count) >= (capacity)) { \
            int new_capacity = ((capacity) == 0) ? 16 : ((capacity) * 2); \
            void *new_ptr = realloc((array), sizeof(type) * (size_t)new_capacity); \
            if (!new_ptr) { \
                fprintf(stderr, "Viewer: out of memory growing %s\n", #array); \
                exit(1); \
            } \
            (array) = (type *)new_ptr; \
            (capacity) = new_capacity; \
        } \
    } while (0)

typedef struct {
    char *name;
    char *expr;
    int value;
    bool has_value;
    bool resolving;
} ViewerSymbol;

typedef struct {
    char *text;
    char *label;
    char *op;
    char *args;
} ViewerAsmLine;

typedef struct {
    const char *path;
    ViewerAsmLine *lines;
    int line_count;
    ViewerSymbol *symbols;
    int symbol_count;
    int symbol_capacity;
} ViewerAsmFile;

typedef struct {
    float x;
    float y;
    float z;
} ViewerVertex;

typedef struct {
    uint16 indices[VIEWER_MAX_FACE_VERTS];
    uint8 count;
    uint8 color_index;
    int16 vis_index;
    int16 group_index;
    int16 nx;
    int16 ny;
    int16 nz;
} ViewerPolyFace;

typedef struct {
    uint16 a;
    uint16 b;
    uint8 color_index;
    int16 vis_index;
    int16 group_index;
} ViewerLineFace;

typedef struct {
    ViewerVertex *vertices;
    int vertex_count;
} ViewerFrame;

typedef struct {
    uint16 p1;
    uint16 p2;
    uint16 p3;
} ViewerVizEntry;

typedef struct {
    char *label;
    int poly_start;
    int poly_count;
    int line_start;
    int line_count;
} ViewerFaceGroup;

typedef struct {
    char *label;
    char *face_group_label;
    char *right_label;
    int16 vis_index;
    int16 face_group_index;
    int16 right_node_index;
} ViewerBspNode;

typedef struct {
    GLuint poly_vao;
    GLuint poly_vbo;
    GLuint poly_color_vbo;
    GLuint line_vao;
    GLuint line_vbo;
    int poly_vertex_count;
    int line_vertex_count;
    int *poly_tri_start;
    int *poly_tri_count;
    int uploaded_frame;
    bool ready;
} ViewerGpuShape;

typedef struct {
    ViewerShapeInfo info;
    int color_table_id;
    ViewerFrame *frames;
    int frame_count;
    ViewerPolyFace *poly_faces;
    int poly_face_count;
    int poly_face_capacity;
    ViewerLineFace *line_faces;
    int line_face_count;
    int line_face_capacity;
    ViewerVizEntry *viz_entries;
    int viz_count;
    int viz_capacity;
    ViewerFaceGroup *face_groups;
    int face_group_count;
    int face_group_capacity;
    ViewerBspNode *bsp_nodes;
    int bsp_node_count;
    int bsp_node_capacity;
    int bsp_root_index;
    float *vertex_normals;
    int vertex_normal_count;
    ViewerGpuShape gpu;
} ViewerShape;

typedef struct {
    char *label;
    uint16 *frames;
    int frame_count;
    int frame_capacity;
} ViewerAnimTable;

typedef struct {
    char *label;
    uint16 *materials;
    int material_count;
    int material_capacity;
    bool has_textures;
} ViewerColorTable;

typedef struct {
    char *label;
    uint8 *pairs;
    int pair_count;
    int pair_capacity;
} ViewerDepthTable;

static ViewerAsmFile s_shape_files[9];
static int s_shape_file_count = 0;
static ViewerAsmFile s_coltabs_file;
static ViewerAsmFile s_coltab_file;
static ViewerAsmFile s_light_file;

static ViewerShape *s_shapes = NULL;
static int s_shape_count = 0;
static int s_shape_capacity = 0;
static int s_shape_header_count = 0;

static ViewerSkippedShapeInfo *s_skipped_shapes = NULL;
static int s_skipped_shape_count = 0;
static int s_skipped_shape_capacity = 0;

static ViewerAnimTable *s_anim_tables = NULL;
static int s_anim_table_count = 0;
static int s_anim_table_capacity = 0;

static ViewerColorTable *s_color_tables = NULL;
static int s_color_table_count = 0;
static int s_color_table_capacity = 0;

static ViewerDepthTable *s_depth_tables = NULL;
static int s_depth_table_count = 0;
static int s_depth_table_capacity = 0;

static uint16 s_norm_palette[16];
static uint16 s_red_palette[16];
static uint16 s_blue_palette[16];
static ViewerPaletteKind s_palette_kind = VIEWER_PALETTE_NORM;
static int s_shade_table_index = 0;

static int s_depth_norm = -1;
static int s_depth_red = -1;
static int s_depth_blue = -1;
static bool s_light_tables_ready = false;
static uint8 s_light_pairs[4][12][10];
static const float s_default_light_dir[3] = {
    0.57735026919f, 0.57735026919f, 0.57735026919f
};
static GLuint s_vertex_color_shader = 0;

static const char *s_vertex_color_vert_src =
    "#version 330 core\n"
    "layout(location = 0) in vec3 aPos;\n"
    "layout(location = 1) in vec4 aColor;\n"
    "uniform mat4 uModel;\n"
    "uniform mat4 uView;\n"
    "uniform mat4 uProj;\n"
    "out vec4 vColor;\n"
    "void main() {\n"
    "    gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);\n"
    "    vColor = aColor;\n"
    "}\n";

static const char *s_vertex_color_frag_src =
    "#version 330 core\n"
    "in vec4 vColor;\n"
    "out vec4 FragColor;\n"
    "void main() {\n"
    "    FragColor = vColor;\n"
    "}\n";

static int viewer_stricmp(const char *a, const char *b) {
    while (*a && *b) {
        int ca = tolower((unsigned char)*a);
        int cb = tolower((unsigned char)*b);
        if (ca != cb) {
            return ca - cb;
        }
        ++a;
        ++b;
    }
    return tolower((unsigned char)*a) - tolower((unsigned char)*b);
}

static bool viewer_startswith_ci(const char *s, const char *prefix) {
    while (*prefix) {
        if (tolower((unsigned char)*s) != tolower((unsigned char)*prefix)) {
            return false;
        }
        ++s;
        ++prefix;
    }
    return true;
}

static char *viewer_strdup(const char *s) {
    size_t len = strlen(s);
    char *copy = (char *)malloc(len + 1u);
    if (!copy) {
        fprintf(stderr, "Viewer: out of memory duplicating string\n");
        exit(1);
    }
    memcpy(copy, s, len + 1u);
    return copy;
}

static char *viewer_strndup_local(const char *s, size_t len) {
    char *copy = (char *)malloc(len + 1u);
    if (!copy) {
        fprintf(stderr, "Viewer: out of memory duplicating string slice\n");
        exit(1);
    }
    memcpy(copy, s, len);
    copy[len] = '\0';
    return copy;
}

static char *viewer_trim(char *s) {
    char *end;
    while (*s && isspace((unsigned char)*s)) {
        ++s;
    }
    end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) {
        --end;
    }
    *end = '\0';
    return s;
}

static void viewer_strip_comment(char *s) {
    bool in_angle = false;
    while (*s) {
        if (*s == '<') {
            in_angle = true;
        } else if (*s == '>') {
            in_angle = false;
        } else if (*s == ';' && !in_angle) {
            *s = '\0';
            return;
        }
        ++s;
    }
}

static bool viewer_is_known_op(const char *token) {
    static const char *ops[] = {
        "shapehdr", "shapehdr_s", "pointsb", "pointsw", "pointsxb", "pointsxw",
        "pb", "pw", "pbd2", "pby2", "frames", "jumptab", "jump",
        "endpoints", "faces", "face2", "face3", "face4", "face5", "face6",
        "face7", "face8", "face9", "face10", "face11", "face12",
        "aface3", "aface4", "fend", "fendq",
        "endshape", "vizis", "viz", "bspinit", "bsp", "bspe", "bspnull", "bspend",
        "datahdr", "db", "dbh", "collite", "coldepth", "colnorm", "colanim",
        "coltext", "colsmooth", "rept", "endr", "ifeq", "ifne", "elseif", "endc",
        "public", "extern", "incfile", "incpublics"
    };
    size_t i;
    for (i = 0; i < ARRAY_SIZE(ops); ++i) {
        if (viewer_stricmp(token, ops[i]) == 0) {
            return true;
        }
    }
    return false;
}

static void viewer_parse_line(ViewerAsmLine *out, char *buffer) {
    char *s;
    char *first;
    char *second;
    char *rest;

    memset(out, 0, sizeof(*out));
    viewer_strip_comment(buffer);
    s = viewer_trim(buffer);
    if (*s == '\0') {
        out->text = viewer_strdup("");
        return;
    }

    out->text = viewer_strdup(s);

    first = s;
    while (*s && !isspace((unsigned char)*s) && *s != '=') {
        ++s;
    }
    if (*s == '=') {
        *s = '\0';
        out->label = viewer_strdup(first);
        out->op = viewer_strdup("=");
        out->args = viewer_strdup(viewer_trim(s + 1));
        return;
    }
    if (*s == '\0') {
        if (viewer_is_known_op(first)) {
            out->op = viewer_strdup(first);
        } else {
            out->label = viewer_strdup(first);
        }
        return;
    }
    *s++ = '\0';
    second = viewer_trim(s);
    rest = second;
    while (*rest && !isspace((unsigned char)*rest) && *rest != '=') {
        ++rest;
    }

    if (*rest == '=') {
        *rest = '\0';
        out->label = viewer_strdup(first);
        out->op = viewer_strdup(second);
        out->args = viewer_strdup(viewer_trim(rest + 1));
        return;
    }

    if (*rest != '\0') {
        *rest++ = '\0';
        rest = viewer_trim(rest);
    }

    if (viewer_is_known_op(first)) {
        out->op = viewer_strdup(first);
        if (*second) {
            char *tmp = (char *)malloc(strlen(second) + (rest ? strlen(rest) : 0u) + 2u);
            if (!tmp) {
                fprintf(stderr, "Viewer: out of memory joining args\n");
                exit(1);
            }
            if (*rest) {
                sprintf(tmp, "%s %s", second, rest);
            } else {
                strcpy(tmp, second);
            }
            out->args = tmp;
        }
        return;
    }

    out->label = viewer_strdup(first);
    out->op = viewer_strdup(second);
    if (*rest) {
        out->args = viewer_strdup(rest);
    }
}

static bool viewer_load_asm_file(ViewerAsmFile *file, const char *path) {
    FILE *fp;
    char linebuf[2048];
    int line_capacity = 0;

    memset(file, 0, sizeof(*file));
    file->path = path;

    fp = fopen(path, "rb");
    if (!fp) {
        fprintf(stderr, "Viewer: failed to open %s\n", path);
        return false;
    }

    while (fgets(linebuf, sizeof(linebuf), fp)) {
        ViewerAsmLine parsed;
        VIEWER_ARRAY_GROW(file->lines, file->line_count, line_capacity, ViewerAsmLine);
        viewer_parse_line(&parsed, linebuf);
        file->lines[file->line_count++] = parsed;
    }

    fclose(fp);
    return true;
}

static void viewer_free_asm_file(ViewerAsmFile *file) {
    int i;
    if (!file) {
        return;
    }
    for (i = 0; i < file->line_count; ++i) {
        free(file->lines[i].text);
        free(file->lines[i].label);
        free(file->lines[i].op);
        free(file->lines[i].args);
    }
    for (i = 0; i < file->symbol_count; ++i) {
        free(file->symbols[i].name);
        free(file->symbols[i].expr);
    }
    free(file->lines);
    free(file->symbols);
    memset(file, 0, sizeof(*file));
}

static ViewerSymbol *viewer_get_symbol(ViewerAsmFile *file, const char *name, bool create) {
    int i;
    for (i = 0; i < file->symbol_count; ++i) {
        if (viewer_stricmp(file->symbols[i].name, name) == 0) {
            return &file->symbols[i];
        }
    }
    if (!create) {
        return NULL;
    }
    VIEWER_ARRAY_GROW(file->symbols, file->symbol_count, file->symbol_capacity, ViewerSymbol);
    file->symbols[file->symbol_count].name = viewer_strdup(name);
    file->symbols[file->symbol_count].expr = NULL;
    file->symbols[file->symbol_count].value = 0;
    file->symbols[file->symbol_count].has_value = false;
    file->symbols[file->symbol_count].resolving = false;
    return &file->symbols[file->symbol_count++];
}

typedef struct {
    const char *s;
    ViewerAsmFile *file;
} ViewerExprParser;

static void viewer_expr_skip_ws(ViewerExprParser *p) {
    while (*p->s && isspace((unsigned char)*p->s)) {
        ++p->s;
    }
}

static int viewer_eval_expression(ViewerExprParser *p, bool *ok);

static int viewer_eval_symbol(ViewerAsmFile *file, const char *name, bool *ok) {
    ViewerSymbol *symbol = viewer_get_symbol(file, name, false);
    ViewerExprParser parser;
    if (!symbol || !symbol->expr) {
        *ok = false;
        return 0;
    }
    if (symbol->has_value) {
        return symbol->value;
    }
    if (symbol->resolving) {
        *ok = false;
        return 0;
    }
    symbol->resolving = true;
    parser.s = symbol->expr;
    parser.file = file;
    symbol->value = viewer_eval_expression(&parser, ok);
    symbol->has_value = *ok;
    symbol->resolving = false;
    return symbol->value;
}

static int viewer_eval_factor(ViewerExprParser *p, bool *ok) {
    int sign = 1;
    int value = 0;
    viewer_expr_skip_ws(p);
    while (*p->s == '+' || *p->s == '-') {
        if (*p->s == '-') {
            sign = -sign;
        }
        ++p->s;
        viewer_expr_skip_ws(p);
    }
    if (*p->s == '(') {
        ++p->s;
        value = viewer_eval_expression(p, ok);
        viewer_expr_skip_ws(p);
        if (*p->s == ')') {
            ++p->s;
        }
        return sign * value;
    }
    if (*p->s == '$') {
        char *endptr;
        ++p->s;
        value = (int)strtol(p->s, &endptr, 16);
        if (endptr == p->s) {
            *ok = false;
            return 0;
        }
        p->s = endptr;
        return sign * value;
    }
    if (isdigit((unsigned char)*p->s)) {
        const char *start = p->s;
        bool has_hex_alpha = false;
        char *endptr;
        while (isalnum((unsigned char)*p->s)) {
            if (isalpha((unsigned char)*p->s)) {
                has_hex_alpha = true;
            }
            ++p->s;
        }
        {
            size_t len = (size_t)(p->s - start);
            char temp[64];
            if (len >= sizeof(temp)) {
                len = sizeof(temp) - 1u;
            }
            memcpy(temp, start, len);
            temp[len] = '\0';
            value = (int)strtol(temp, &endptr, has_hex_alpha ? 16 : 10);
            (void)endptr;
        }
        return sign * value;
    }
    if (isalpha((unsigned char)*p->s) || *p->s == '_' || *p->s == '.') {
        const char *start = p->s;
        char name[128];
        size_t len;
        while (isalnum((unsigned char)*p->s) || *p->s == '_' || *p->s == '.') {
            ++p->s;
        }
        len = (size_t)(p->s - start);
        if (len >= sizeof(name)) {
            len = sizeof(name) - 1u;
        }
        memcpy(name, start, len);
        name[len] = '\0';
        value = viewer_eval_symbol(p->file, name, ok);
        if (!*ok) {
            return 0;
        }
        return sign * value;
    }
    *ok = false;
    return 0;
}

static int viewer_eval_term(ViewerExprParser *p, bool *ok) {
    int value = viewer_eval_factor(p, ok);
    while (*ok) {
        int rhs;
        viewer_expr_skip_ws(p);
        if (*p->s != '*' && *p->s != '/') {
            break;
        }
        if (*p->s == '*') {
            ++p->s;
            rhs = viewer_eval_factor(p, ok);
            value *= rhs;
        } else {
            ++p->s;
            rhs = viewer_eval_factor(p, ok);
            if (rhs == 0) {
                *ok = false;
                return 0;
            }
            value /= rhs;
        }
    }
    return value;
}

static int viewer_eval_expression(ViewerExprParser *p, bool *ok) {
    int value = viewer_eval_term(p, ok);
    while (*ok) {
        int rhs;
        viewer_expr_skip_ws(p);
        if (*p->s != '+' && *p->s != '-') {
            break;
        }
        if (*p->s == '+') {
            ++p->s;
            rhs = viewer_eval_term(p, ok);
            value += rhs;
        } else {
            ++p->s;
            rhs = viewer_eval_term(p, ok);
            value -= rhs;
        }
    }
    return value;
}

static int viewer_eval_expr_string(ViewerAsmFile *file, const char *expr, bool *ok) {
    ViewerExprParser parser;
    parser.s = expr;
    parser.file = file;
    *ok = true;
    return viewer_eval_expression(&parser, ok);
}

static void viewer_collect_symbols(ViewerAsmFile *file) {
    int i;
    for (i = 0; i < file->line_count; ++i) {
        ViewerAsmLine *line = &file->lines[i];
        ViewerSymbol *symbol;
        if (!line->label || !line->op || !line->args) {
            continue;
        }
        if (viewer_stricmp(line->op, "equ") != 0 && strcmp(line->op, "=") != 0) {
            continue;
        }
        symbol = viewer_get_symbol(file, line->label, true);
        free(symbol->expr);
        symbol->expr = viewer_strdup(line->args);
        symbol->has_value = false;
        symbol->resolving = false;
    }
}

static char **viewer_split_args(const char *args, int *out_count) {
    char **parts = NULL;
    int count = 0;
    int capacity = 0;
    const char *p = args ? args : "";
    const char *start = p;
    int paren_depth = 0;
    int angle_depth = 0;

    while (1) {
        bool at_end = (*p == '\0');
        bool is_sep = (*p == ',' && paren_depth == 0 && angle_depth == 0);
        if (at_end || is_sep) {
            size_t len = (size_t)(p - start);
            char *part = (char *)malloc(len + 1u);
            char *trimmed;
            if (!part) {
                fprintf(stderr, "Viewer: out of memory splitting args\n");
                exit(1);
            }
            memcpy(part, start, len);
            part[len] = '\0';
            trimmed = viewer_trim(part);
            VIEWER_ARRAY_GROW(parts, count, capacity, char *);
            parts[count++] = viewer_strdup(trimmed);
            free(part);
            if (at_end) {
                break;
            }
            start = p + 1;
        } else if (*p == '(') {
            ++paren_depth;
        } else if (*p == ')' && paren_depth > 0) {
            --paren_depth;
        } else if (*p == '<') {
            ++angle_depth;
        } else if (*p == '>' && angle_depth > 0) {
            --angle_depth;
        }
        ++p;
    }

    *out_count = count;
    return parts;
}

static void viewer_free_split_args(char **parts, int count) {
    int i;
    for (i = 0; i < count; ++i) {
        free(parts[i]);
    }
    free(parts);
}

static int viewer_find_label_target(const ViewerAsmFile *file, int start_line, int end_line, const char *label) {
    int i;
    for (i = start_line; i < end_line; ++i) {
        const ViewerAsmLine *line = &file->lines[i];
        if (line->label && viewer_stricmp(line->label, label) == 0) {
            if (!line->op && i + 1 < end_line) {
                return i + 1;
            }
            return i;
        }
    }
    return -1;
}

static const char *viewer_attached_label(const ViewerAsmFile *file, int line_index) {
    int i;
    const ViewerAsmLine *line = &file->lines[line_index];
    if (line->label) {
        return line->label;
    }
    for (i = line_index - 1; i >= 0; --i) {
        const ViewerAsmLine *prev = &file->lines[i];
        if (prev->label && !prev->op) {
            return prev->label;
        }
        if (!prev->op || (prev->text && prev->text[0] == '\0')) {
            continue;
        }
        if (viewer_stricmp(prev->op, "=") == 0 ||
            viewer_stricmp(prev->op, "ifne") == 0 ||
            viewer_stricmp(prev->op, "ifeq") == 0 ||
            viewer_stricmp(prev->op, "elseif") == 0 ||
            viewer_stricmp(prev->op, "endc") == 0) {
            continue;
        }
        break;
    }
    return NULL;
}

static bool viewer_load_palette_file(const char *path, uint16 out_palette[16]) {
    FILE *fp = fopen(path, "rb");
    uint8 data[256 * 2];
    size_t read_count;
    int i;
    const size_t row_offset = 7u * 32u;
    if (!fp) {
        fprintf(stderr, "Viewer: failed to open palette %s\n", path);
        return false;
    }
    read_count = fread(data, 1, sizeof(data), fp);
    fclose(fp);
    if (read_count < row_offset + 32u) {
        fprintf(stderr, "Viewer: palette file too small %s\n", path);
        return false;
    }
    for (i = 0; i < 16; ++i) {
        size_t off = row_offset + (size_t)(i * 2);
        out_palette[i] = (uint16)(data[off] | ((uint16)data[off + 1] << 8));
    }
    return true;
}

static bool viewer_parse_light_tables(void) {
    int i;
    int loaded_rows = 0;
    memset(s_light_pairs, 0, sizeof(s_light_pairs));

    for (i = 0; i < s_light_file.line_count; ++i) {
        ViewerAsmLine *line = &s_light_file.lines[i];
        int set_index;
        int row_index;
        int arg_count;
        char **args;
        int shade_index;

        if (!line->label || !line->op || viewer_stricmp(line->op, "db") != 0) {
            continue;
        }
        if (sscanf(line->label, "shades%d_%d", &set_index, &row_index) != 2) {
            continue;
        }
        if (set_index < 0 || set_index >= 4 || row_index < 0 || row_index >= 10) {
            continue;
        }

        args = viewer_split_args(line->args, &arg_count);
        if (arg_count < 10) {
            viewer_free_split_args(args, arg_count);
            continue;
        }
        for (shade_index = 0; shade_index < 10; ++shade_index) {
            bool ok = true;
            int value = viewer_eval_expr_string(&s_light_file, args[shade_index], &ok);
            if (!ok) {
                value = 0;
            }
            s_light_pairs[set_index][row_index][shade_index] = (uint8)(value & 0xFF);
        }
        viewer_free_split_args(args, arg_count);
        ++loaded_rows;
    }

    for (i = 0; i < 4; ++i) {
        memcpy(s_light_pairs[i][10], s_light_pairs[i][9], 10u);
        memcpy(s_light_pairs[i][11], s_light_pairs[i][9], 10u);
    }

    s_light_tables_ready = loaded_rows == 40;
    if (!s_light_tables_ready) {
        fprintf(stderr, "Viewer: expected 40 LIGHT.ASM shade rows, loaded %d\n", loaded_rows);
    }
    return s_light_tables_ready;
}

static void viewer_decode_bgr555(uint16 color, float *r, float *g, float *b) {
    *r = (float)(color & 0x1Fu) / 31.0f;
    *g = (float)((color >> 5) & 0x1Fu) / 31.0f;
    *b = (float)((color >> 10) & 0x1Fu) / 31.0f;
}

static const uint16 *viewer_get_active_palette(void) {
    switch (s_palette_kind) {
    case VIEWER_PALETTE_RED:
        return s_red_palette;
    case VIEWER_PALETTE_BLUE:
        return s_blue_palette;
    case VIEWER_PALETTE_NORM:
    default:
        return s_norm_palette;
    }
}

static int viewer_get_active_depth_table_id(void) {
    switch (s_palette_kind) {
    case VIEWER_PALETTE_RED:
        return s_depth_red;
    case VIEWER_PALETTE_BLUE:
        return s_depth_blue;
    case VIEWER_PALETTE_NORM:
    default:
        return s_depth_norm;
    }
}

static int viewer_find_depth_table(const char *label) {
    int i;
    for (i = 0; i < s_depth_table_count; ++i) {
        if (viewer_stricmp(s_depth_tables[i].label, label) == 0) {
            return i;
        }
    }
    return -1;
}

static int viewer_parse_depth_table(const char *label) {
    int start = viewer_find_label_target(&s_coltab_file, 0, s_coltab_file.line_count, label);
    int i;
    ViewerDepthTable *table;
    if (start < 0) {
        return -1;
    }
    if (viewer_find_depth_table(label) >= 0) {
        return viewer_find_depth_table(label);
    }
    VIEWER_ARRAY_GROW(s_depth_tables, s_depth_table_count, s_depth_table_capacity, ViewerDepthTable);
    table = &s_depth_tables[s_depth_table_count];
    memset(table, 0, sizeof(*table));
    table->label = viewer_strdup(label);
    for (i = start; i < s_coltab_file.line_count; ++i) {
        ViewerAsmLine *line = &s_coltab_file.lines[i];
        int arg_count;
        char **args;
        int j;
        if (i > start && line->label) {
            break;
        }
        if (!line->op || viewer_stricmp(line->op, "dbh") != 0) {
            continue;
        }
        args = viewer_split_args(line->args, &arg_count);
        for (j = 0; j < arg_count; ++j) {
            bool ok = true;
            int value = viewer_eval_expr_string(&s_coltab_file, args[j], &ok);
            if (!ok) {
                char prefixed[64];
                snprintf(prefixed, sizeof(prefixed), "$%s", args[j]);
                value = viewer_eval_expr_string(&s_coltab_file, prefixed, &ok);
            }
            if (!ok) {
                value = 0;
            }
            VIEWER_ARRAY_GROW(table->pairs, table->pair_count, table->pair_capacity, uint8);
            table->pairs[table->pair_count++] = (uint8)(value & 0xFF);
        }
        viewer_free_split_args(args, arg_count);
    }
    return s_depth_table_count++;
}

static int viewer_find_anim_table(const char *label) {
    int i;
    for (i = 0; i < s_anim_table_count; ++i) {
        if (viewer_stricmp(s_anim_tables[i].label, label) == 0) {
            return i;
        }
    }
    return -1;
}

static int viewer_find_color_table(const char *label) {
    int i;
    for (i = 0; i < s_color_table_count; ++i) {
        if (viewer_stricmp(s_color_tables[i].label, label) == 0) {
            return i;
        }
    }
    return -1;
}

static bool viewer_is_material_op(const char *op) {
    if (!op) {
        return false;
    }
    return viewer_stricmp(op, "collite") == 0 ||
           viewer_stricmp(op, "coldepth") == 0 ||
           viewer_stricmp(op, "colnorm") == 0 ||
           viewer_stricmp(op, "colanim") == 0 ||
           viewer_stricmp(op, "coltext") == 0 ||
           viewer_stricmp(op, "colsmooth") == 0;
}

static uint16 viewer_parse_material_word(const ViewerAsmLine *line, bool *has_texture);

static int viewer_parse_anim_table(const char *label) {
    int start = viewer_find_label_target(&s_coltabs_file, 0, s_coltabs_file.line_count, label);
    int table_index;
    int i;
    int expected = 0;
    int existing = viewer_find_anim_table(label);
    if (existing >= 0) {
        return existing;
    }
    if (start < 0) {
        fprintf(stderr, "Viewer: missing animation table %s\n", label);
        return -1;
    }
    VIEWER_ARRAY_GROW(s_anim_tables, s_anim_table_count, s_anim_table_capacity, ViewerAnimTable);
    table_index = s_anim_table_count;
    memset(&s_anim_tables[table_index], 0, sizeof(s_anim_tables[table_index]));
    s_anim_tables[table_index].label = viewer_strdup(label);
    ++s_anim_table_count;

    for (i = start; i < s_coltabs_file.line_count; ++i) {
        ViewerAsmLine *line = &s_coltabs_file.lines[i];
        if (i > start && line->label && !viewer_is_material_op(line->op) && (!line->op || viewer_stricmp(line->op, "db") != 0)) {
            break;
        }
        if (!line->op) {
            continue;
        }
        if (viewer_stricmp(line->op, "db") == 0 && expected == 0) {
            bool ok = true;
            expected = viewer_eval_expr_string(&s_coltabs_file, line->args, &ok);
            if (!ok || expected < 0) {
                expected = 0;
            }
            continue;
        }
        if (viewer_is_material_op(line->op)) {
            ViewerAnimTable *table = &s_anim_tables[table_index];
            bool has_texture = false;
            uint16 material = viewer_parse_material_word(line, &has_texture);
            table = &s_anim_tables[table_index];
            VIEWER_ARRAY_GROW(table->frames, table->frame_count, table->frame_capacity, uint16);
            table->frames[table->frame_count++] = material;
            if (expected > 0 && table->frame_count >= expected) {
                break;
            }
        }
    }

    return table_index;
}

static void viewer_parse_material_block(ViewerColorTable *table, int *io_line);

static int viewer_parse_color_table(const char *label) {
    int start = viewer_find_label_target(&s_coltabs_file, 0, s_coltabs_file.line_count, label);
    ViewerColorTable *table;
    int existing = viewer_find_color_table(label);
    int cursor;
    if (existing >= 0) {
        return existing;
    }
    if (start < 0) {
        fprintf(stderr, "Viewer: missing color table %s\n", label);
        return -1;
    }
    VIEWER_ARRAY_GROW(s_color_tables, s_color_table_count, s_color_table_capacity, ViewerColorTable);
    table = &s_color_tables[s_color_table_count];
    memset(table, 0, sizeof(*table));
    table->label = viewer_strdup(label);
    cursor = start;
    viewer_parse_material_block(table, &cursor);
    return s_color_table_count++;
}

static void viewer_append_material(ViewerColorTable *table, uint16 material, bool has_texture) {
    VIEWER_ARRAY_GROW(table->materials, table->material_count, table->material_capacity, uint16);
    table->materials[table->material_count++] = material;
    if (has_texture) {
        table->has_textures = true;
    }
}

static void viewer_parse_material_block(ViewerColorTable *table, int *io_line) {
    int i = *io_line;
    while (i < s_coltabs_file.line_count) {
        ViewerAsmLine *line = &s_coltabs_file.lines[i];
        if (i > *io_line && line->label && !viewer_is_material_op(line->op) && (!line->op || viewer_stricmp(line->op, "rept") != 0) && (!line->op || viewer_stricmp(line->op, "db") != 0)) {
            break;
        }
        if (!line->op) {
            ++i;
            continue;
        }
        if (viewer_stricmp(line->op, "rept") == 0) {
            bool ok = true;
            int repeat = viewer_eval_expr_string(&s_coltabs_file, line->args, &ok);
            int block_start = i + 1;
            int block_end = block_start;
            int depth = 1;
            if (!ok || repeat < 0) {
                repeat = 0;
            }
            while (block_end < s_coltabs_file.line_count && depth > 0) {
                ViewerAsmLine *block_line = &s_coltabs_file.lines[block_end];
                if (block_line->op && viewer_stricmp(block_line->op, "rept") == 0) {
                    ++depth;
                } else if (block_line->op && viewer_stricmp(block_line->op, "endr") == 0) {
                    --depth;
                    if (depth == 0) {
                        break;
                    }
                }
                ++block_end;
            }
            while (repeat-- > 0) {
                int replay = block_start;
                while (replay < block_end) {
                    ViewerAsmLine *replay_line = &s_coltabs_file.lines[replay];
                    if (viewer_is_material_op(replay_line->op)) {
                        bool has_texture = false;
                        viewer_append_material(table, viewer_parse_material_word(replay_line, &has_texture), has_texture);
                    }
                    ++replay;
                }
            }
            i = block_end + 1;
            continue;
        }
        if (viewer_is_material_op(line->op)) {
            bool has_texture = false;
            viewer_append_material(table, viewer_parse_material_word(line, &has_texture), has_texture);
        }
        ++i;
    }
    *io_line = i;
}

static uint16 viewer_parse_material_word(const ViewerAsmLine *line, bool *has_texture) {
    int arg_count = 0;
    char **args = viewer_split_args(line->args, &arg_count);
    uint16 word = 0;
    bool ok = true;
    *has_texture = false;

    if (viewer_stricmp(line->op, "collite") == 0 && arg_count >= 2) {
        word = MATERIAL_COLLITE((uint16)viewer_eval_expr_string(&s_coltabs_file, args[0], &ok),
                                (uint16)viewer_eval_expr_string(&s_coltabs_file, args[1], &ok));
    } else if (viewer_stricmp(line->op, "coldepth") == 0 && arg_count >= 1) {
        word = MATERIAL_COLDEPTH((uint16)viewer_eval_expr_string(&s_coltabs_file, args[0], &ok));
    } else if (viewer_stricmp(line->op, "colnorm") == 0 && arg_count >= 1) {
        uint16 lo = (uint16)viewer_eval_expr_string(&s_coltabs_file, args[0], &ok);
        uint16 hi = (arg_count >= 2) ? (uint16)viewer_eval_expr_string(&s_coltabs_file, args[1], &ok) : lo;
        word = MATERIAL_COLNORM(lo, hi);
    } else if (viewer_stricmp(line->op, "colanim") == 0 && arg_count >= 1) {
        int anim_id = viewer_parse_anim_table(args[0]);
        word = MATERIAL_COLANIM(anim_id >= 0 ? (uint16)anim_id : 0u);
    } else if (viewer_stricmp(line->op, "coltext") == 0) {
        uint16 tex_id = 0u;
        uint16 tex_xy = 0u;
        if (arg_count >= 2) {
            tex_xy = (uint16)viewer_eval_expr_string(&s_coltabs_file, args[1], &ok);
        }
        word = MATERIAL_COLTEXT(tex_id, tex_xy);
        *has_texture = true;
    } else if (viewer_stricmp(line->op, "colsmooth") == 0 && arg_count >= 2) {
        word = MATERIAL_COLSMOOTH((uint16)viewer_eval_expr_string(&s_coltabs_file, args[0], &ok),
                                  (uint16)viewer_eval_expr_string(&s_coltabs_file, args[1], &ok));
    }

    if (!ok) {
        word = MATERIAL_COLNORM(0xFu, 0x0u);
    }
    viewer_free_split_args(args, arg_count);
    return word;
}

static void viewer_decode_palette_pair(uint8 pair, float out[4]) {
    const uint16 *palette = viewer_get_active_palette();
    uint8 lo = (uint8)(pair & 0x0Fu);
    uint8 hi = (uint8)((pair >> 4) & 0x0Fu);
    float r0, g0, b0;
    float r1, g1, b1;
    viewer_decode_bgr555(palette[lo], &r0, &g0, &b0);
    viewer_decode_bgr555(palette[hi], &r1, &g1, &b1);
    out[0] = (r0 + r1) * 0.5f;
    out[1] = (g0 + g1) * 0.5f;
    out[2] = (b0 + b1) * 0.5f;
    out[3] = 1.0f;
}

static int viewer_compute_shade_index_vec3(float fx, float fy, float fz) {
    float len = sqrtf(fx * fx + fy * fy + fz * fz);
    float dot;
    float t;
    int shade_index;

    if (len <= 0.0001f) {
        return 9;
    }

    dot = ((fx / len) * s_default_light_dir[0]) +
          ((fy / len) * s_default_light_dir[1]) +
          ((fz / len) * s_default_light_dir[2]);
    if (dot < -1.0f) dot = -1.0f;
    if (dot > 1.0f) dot = 1.0f;
    t = (dot + 1.0f) * 0.5f;
    shade_index = (int)lroundf(t * 9.0f);
    if (shade_index < 0) shade_index = 0;
    if (shade_index > 9) shade_index = 9;
    return shade_index;
}

static int viewer_compute_shade_index(int16 nx, int16 ny, int16 nz) {
    return viewer_compute_shade_index_vec3((float)nx, (float)ny, (float)nz);
}

static uint16 viewer_resolve_terminal_material(uint16 material, int col_frame) {
    if ((material & 0x8000u) != 0u && (material & 0x4000u) == 0u) {
        uint16 anim_id = (uint16)(material & 0x3FFFu);
        if (anim_id < (uint16)s_anim_table_count && s_anim_tables[anim_id].frame_count > 0) {
            int count = s_anim_tables[anim_id].frame_count;
            int frame = (count > 0) ? (col_frame % count) : 0;
            return viewer_resolve_terminal_material(s_anim_tables[anim_id].frames[frame], col_frame);
        }
    }
    return material;
}

static bool viewer_material_is_smooth(uint16 material) {
    return (material & 0xC000u) == 0xC000u;
}

static void viewer_resolve_material(uint16 material, int col_frame, int shade_index, float out[4]) {
    uint8 source;
    material = viewer_resolve_terminal_material(material, col_frame);
    source = (uint8)((material >> 8) & 0x3Fu);
    if ((material & 0x4000u) != 0u && (material & 0x8000u) == 0u) {
        out[0] = VIEWER_DEBUG_MAGENTA_R;
        out[1] = VIEWER_DEBUG_MAGENTA_G;
        out[2] = VIEWER_DEBUG_MAGENTA_B;
        out[3] = 1.0f;
        return;
    }
    if (source == 63u) {
        viewer_decode_palette_pair((uint8)(material & 0xFFu), out);
        return;
    }
    if (source == 62u) {
        int depth_id = viewer_get_active_depth_table_id();
        uint8 index = (uint8)(material & 0xFFu);
        if (depth_id >= 0 && depth_id < s_depth_table_count && index < (uint8)s_depth_tables[depth_id].pair_count) {
            viewer_decode_palette_pair(s_depth_tables[depth_id].pairs[index], out);
            return;
        }
        out[0] = VIEWER_DEBUG_MAGENTA_R;
        out[1] = VIEWER_DEBUG_MAGENTA_G;
        out[2] = VIEWER_DEBUG_MAGENTA_B;
        out[3] = 1.0f;
        return;
    }

    if (s_light_tables_ready && source < 12u) {
        int table_index = s_shade_table_index;
        if (table_index < 0) table_index = 0;
        if (table_index > 3) table_index = 3;
        if (shade_index < 0) shade_index = 0;
        if (shade_index > 9) shade_index = 9;
        viewer_decode_palette_pair(s_light_pairs[table_index][source][shade_index], out);
        return;
    }

    out[0] = VIEWER_DEBUG_MAGENTA_R;
    out[1] = VIEWER_DEBUG_MAGENTA_G;
    out[2] = VIEWER_DEBUG_MAGENTA_B;
    out[3] = 1.0f;
}

static bool viewer_get_face_material(const ViewerShape *shape, uint8 color_index,
                                     int col_frame, uint16 *out_material) {
    if (shape->color_table_id < 0 || shape->color_table_id >= s_color_table_count) {
        return false;
    }
    if ((int)color_index >= s_color_tables[shape->color_table_id].material_count) {
        return false;
    }
    *out_material = viewer_resolve_terminal_material(
        s_color_tables[shape->color_table_id].materials[color_index], col_frame);
    return true;
}

static void viewer_resolve_face_color(const ViewerShape *shape, uint8 color_index,
                                      int col_frame, int shade_index, float out[4]) {
    uint16 material = 0;
    if (!viewer_get_face_material(shape, color_index, col_frame, &material)) {
        out[0] = VIEWER_DEBUG_MAGENTA_R;
        out[1] = VIEWER_DEBUG_MAGENTA_G;
        out[2] = VIEWER_DEBUG_MAGENTA_B;
        out[3] = 1.0f;
        return;
    }
    viewer_resolve_material(material, col_frame, shade_index, out);
}

static void viewer_append_vertex(ViewerVertex **vertices, int *count, int *capacity, float x, float y, float z) {
    VIEWER_ARRAY_GROW(*vertices, *count, *capacity, ViewerVertex);
    (*vertices)[*count].x = x;
    (*vertices)[*count].y = y;
    (*vertices)[*count].z = z;
    ++(*count);
}

static int viewer_parse_point_line(ViewerAsmFile *file, const ViewerAsmLine *line,
                                   ViewerVertex **vertices, int *count, int *capacity,
                                   bool mirrored) {
    int arg_count;
    char **args = viewer_split_args(line->args, &arg_count);
    bool ok = true;
    int x = 0;
    int y = 0;
    int z = 0;
    if (arg_count >= 3) {
        x = viewer_eval_expr_string(file, args[0], &ok);
        y = viewer_eval_expr_string(file, args[1], &ok);
        z = viewer_eval_expr_string(file, args[2], &ok);
    }
    if (!ok) {
        x = y = z = 0;
    }
    if (viewer_stricmp(line->op, "pbd2") == 0) {
        x /= 2;
        y /= 2;
        z /= 2;
    } else if (viewer_stricmp(line->op, "pby2") == 0) {
        y *= 2;
    }
    viewer_append_vertex(vertices, count, capacity, (float)x, (float)y, (float)z);
    if (mirrored) {
        viewer_append_vertex(vertices, count, capacity, (float)(-x), (float)y, (float)z);
    }
    viewer_free_split_args(args, arg_count);
    return mirrored ? 2 : 1;
}

static bool viewer_build_shape_frames(ViewerAsmFile *file, const char *points_label,
                                      int shift, ViewerFrame **out_frames, int *out_frame_count) {
    int start = viewer_find_label_target(file, 0, file->line_count, points_label);
    int end = -1;
    int i;
    int frame_count = 1;
    ViewerFrame *frames = NULL;
    int frame_capacity = 0;
    if (start < 0) {
        return false;
    }
    for (i = start; i < file->line_count; ++i) {
        if (file->lines[i].op && viewer_stricmp(file->lines[i].op, "endpoints") == 0) {
            end = i + 1;
            break;
        }
    }
    if (end < 0) {
        return false;
    }
    for (i = start; i < end; ++i) {
        if (file->lines[i].op && viewer_stricmp(file->lines[i].op, "frames") == 0) {
            bool ok = true;
            int count = viewer_eval_expr_string(file, file->lines[i].args, &ok);
            if (ok && count > frame_count) {
                frame_count = count;
            }
        }
    }
    for (i = 0; i < frame_count; ++i) {
        int pc = start;
        int guard = 0;
        int point_capacity = 0;
        ViewerVertex *vertices = NULL;
        int vertex_count = 0;
        int pending_count = 0;
        bool pending_mirror = false;
        while (pc >= start && pc < end && guard++ < (end - start) * 64) {
            ViewerAsmLine *line = &file->lines[pc];
            if (!line->op) {
                ++pc;
                continue;
            }
            if (viewer_stricmp(line->op, "endpoints") == 0) {
                break;
            }
            if (viewer_stricmp(line->op, "pointsb") == 0 || viewer_stricmp(line->op, "pointsw") == 0 ||
                viewer_stricmp(line->op, "pointsxb") == 0 || viewer_stricmp(line->op, "pointsxw") == 0) {
                bool ok = true;
                pending_count = viewer_eval_expr_string(file, line->args, &ok);
                if (!ok) {
                    pending_count = 0;
                }
                pending_mirror = viewer_stricmp(line->op, "pointsxb") == 0 || viewer_stricmp(line->op, "pointsxw") == 0;
                ++pc;
                continue;
            }
            if (viewer_stricmp(line->op, "pb") == 0 || viewer_stricmp(line->op, "pw") == 0 ||
                viewer_stricmp(line->op, "pbd2") == 0 || viewer_stricmp(line->op, "pby2") == 0) {
                if (pending_count > 0) {
                    viewer_parse_point_line(file, line, &vertices, &vertex_count, &point_capacity, pending_mirror);
                    --pending_count;
                }
                ++pc;
                continue;
            }
            if (viewer_stricmp(line->op, "frames") == 0) {
                bool ok = true;
                int branch_count = viewer_eval_expr_string(file, line->args, &ok);
                int chosen = 0;
                int scan = pc + 1;
                int seen = 0;
                int target = -1;
                if (!ok || branch_count <= 0) {
                    ++pc;
                    continue;
                }
                chosen = i % branch_count;
                while (scan < end && seen < branch_count) {
                    ViewerAsmLine *jump_line = &file->lines[scan];
                    if (jump_line->op && viewer_stricmp(jump_line->op, "jumptab") == 0) {
                        if (seen == chosen) {
                            target = viewer_find_label_target(file, start, end, viewer_trim(jump_line->args));
                            break;
                        }
                        ++seen;
                    }
                    ++scan;
                }
                if (target >= 0) {
                    pc = target;
                } else {
                    pc = scan;
                }
                continue;
            }
            if (viewer_stricmp(line->op, "jump") == 0) {
                int target = viewer_find_label_target(file, start, end, viewer_trim(line->args));
                if (target >= 0) {
                    pc = target;
                    continue;
                }
            }
            ++pc;
        }
        if (shift != 0) {
            float scale = (float)(1 << shift);
            int v;
            for (v = 0; v < vertex_count; ++v) {
                vertices[v].x *= scale;
                vertices[v].y *= scale;
                vertices[v].z *= scale;
            }
        }
        VIEWER_ARRAY_GROW(frames, i, frame_capacity, ViewerFrame);
        frames[i].vertices = vertices;
        frames[i].vertex_count = vertex_count;
    }
    *out_frames = frames;
    *out_frame_count = frame_count;
    return true;
}

static int viewer_append_face_group(ViewerShape *shape, const char *label) {
    ViewerFaceGroup *group;
    VIEWER_ARRAY_GROW(shape->face_groups, shape->face_group_count, shape->face_group_capacity, ViewerFaceGroup);
    group = &shape->face_groups[shape->face_group_count];
    memset(group, 0, sizeof(*group));
    group->label = (label && *label) ? viewer_strdup(label) : NULL;
    group->poly_start = shape->poly_face_count;
    group->line_start = shape->line_face_count;
    return shape->face_group_count++;
}

static int viewer_ensure_face_group(ViewerShape *shape, int current_group, const char *fallback_label) {
    if (current_group >= 0 && current_group < shape->face_group_count) {
        return current_group;
    }
    return viewer_append_face_group(shape, fallback_label);
}

static int viewer_find_face_group_index(const ViewerShape *shape, const char *label) {
    int i;
    if (!shape || !label || !*label) {
        return -1;
    }
    for (i = 0; i < shape->face_group_count; ++i) {
        if (shape->face_groups[i].label && viewer_stricmp(shape->face_groups[i].label, label) == 0) {
            return i;
        }
    }
    return -1;
}

static int viewer_append_bsp_node(ViewerShape *shape,
                                  const char *node_label,
                                  int16 vis_index,
                                  const char *face_group_label,
                                  const char *right_label) {
    ViewerBspNode *node;
    VIEWER_ARRAY_GROW(shape->bsp_nodes, shape->bsp_node_count, shape->bsp_node_capacity, ViewerBspNode);
    node = &shape->bsp_nodes[shape->bsp_node_count];
    memset(node, 0, sizeof(*node));
    node->label = (node_label && *node_label) ? viewer_strdup(node_label) : NULL;
    node->face_group_label = (face_group_label && *face_group_label) ? viewer_strdup(face_group_label) : NULL;
    node->right_label = (right_label && *right_label) ? viewer_strdup(right_label) : NULL;
    node->vis_index = vis_index;
    node->face_group_index = -1;
    node->right_node_index = -1;
    return shape->bsp_node_count++;
}

static int viewer_find_bsp_node_index(const ViewerShape *shape, const char *label) {
    int i;
    if (!shape || !label || !*label) {
        return -1;
    }
    for (i = 0; i < shape->bsp_node_count; ++i) {
        if (shape->bsp_nodes[i].label && viewer_stricmp(shape->bsp_nodes[i].label, label) == 0) {
            return i;
        }
    }
    return -1;
}

static void viewer_resolve_bsp_nodes(ViewerShape *shape) {
    int i;
    if (!shape) {
        return;
    }
    for (i = 0; i < shape->bsp_node_count; ++i) {
        ViewerBspNode *node = &shape->bsp_nodes[i];
        if (node->face_group_label) {
            node->face_group_index = (int16)viewer_find_face_group_index(shape, node->face_group_label);
        }
        if (node->right_label) {
            node->right_node_index = (int16)viewer_find_bsp_node_index(shape, node->right_label);
        }
        if (node->face_group_index < 0 && shape->face_group_count == 1) {
            node->face_group_index = 0;
        }
    }
    if (shape->bsp_root_index < 0 && shape->bsp_node_count > 0) {
        shape->bsp_root_index = 0;
    }
}

static void viewer_append_line_face(ViewerShape *shape, uint16 a, uint16 b, uint8 color_index, int group_index) {
    VIEWER_ARRAY_GROW(shape->line_faces, shape->line_face_count, shape->line_face_capacity, ViewerLineFace);
    shape->line_faces[shape->line_face_count].a = a;
    shape->line_faces[shape->line_face_count].b = b;
    shape->line_faces[shape->line_face_count].color_index = color_index;
    shape->line_faces[shape->line_face_count].vis_index = -1;
    shape->line_faces[shape->line_face_count].group_index = (int16)group_index;
    if (group_index >= 0 && group_index < shape->face_group_count) {
        ++shape->face_groups[group_index].line_count;
    }
    ++shape->line_face_count;
}

static void viewer_append_viz_entry(ViewerShape *shape, uint16 p1, uint16 p2, uint16 p3) {
    VIEWER_ARRAY_GROW(shape->viz_entries, shape->viz_count, shape->viz_capacity, ViewerVizEntry);
    shape->viz_entries[shape->viz_count].p1 = p1;
    shape->viz_entries[shape->viz_count].p2 = p2;
    shape->viz_entries[shape->viz_count].p3 = p3;
    ++shape->viz_count;
}

static bool viewer_build_shape_faces(ViewerAsmFile *file, const char *faces_label, ViewerShape *shape) {
    int start = viewer_find_label_target(file, 0, file->line_count, faces_label);
    int current_group = -1;
    int i;
    if (start < 0) {
        return false;
    }
    for (i = start; i < file->line_count; ++i) {
        ViewerAsmLine *line = &file->lines[i];
        if (line->op && viewer_stricmp(line->op, "endshape") == 0) {
            break;
        }
        if (!line->op) {
            continue;
        }
        if (viewer_stricmp(line->op, "faces") == 0) {
            current_group = viewer_append_face_group(shape, viewer_attached_label(file, i));
            continue;
        }
        if (viewer_stricmp(line->op, "fend") == 0 || viewer_stricmp(line->op, "fendq") == 0) {
            current_group = -1;
            continue;
        }
        if (viewer_stricmp(line->op, "vizis") == 0 ||
            viewer_stricmp(line->op, "bspinit") == 0 ||
            viewer_stricmp(line->op, "bspend") == 0) {
            continue;
        } else if (viewer_stricmp(line->op, "bsp") == 0 || viewer_stricmp(line->op, "bspnull") == 0) {
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if ((viewer_stricmp(line->op, "bsp") == 0 && arg_count >= 3) ||
                (viewer_stricmp(line->op, "bspnull") == 0 && arg_count >= 2)) {
                bool ok = true;
                int16 vis_index = (int16)viewer_eval_expr_string(file, args[0], &ok);
                const char *face_group_label = args[1];
                const char *right_label = (arg_count >= 3) ? args[2] : NULL;
                int node_index;
                if (ok) {
                    node_index = viewer_append_bsp_node(shape, viewer_attached_label(file, i), vis_index,
                                                        face_group_label, right_label);
                    if (shape->bsp_root_index < 0) {
                        shape->bsp_root_index = node_index;
                    }
                }
            }
            viewer_free_split_args(args, arg_count);
            continue;
        } else if (viewer_stricmp(line->op, "bspe") == 0) {
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if (arg_count >= 1) {
                int node_index = viewer_append_bsp_node(shape, viewer_attached_label(file, i), -1, args[0], NULL);
                if (shape->bsp_root_index < 0) {
                    shape->bsp_root_index = node_index;
                }
            }
            viewer_free_split_args(args, arg_count);
            continue;
        } else if (viewer_stricmp(line->op, "viz") == 0) {
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if (arg_count >= 3) {
                bool ok = true;
                uint16 p1 = (uint16)viewer_eval_expr_string(file, args[0], &ok);
                uint16 p2 = (uint16)viewer_eval_expr_string(file, args[1], &ok);
                uint16 p3 = (uint16)viewer_eval_expr_string(file, args[2], &ok);
                if (ok) {
                    viewer_append_viz_entry(shape, p1, p2, p3);
                }
            }
            viewer_free_split_args(args, arg_count);
        } else if (viewer_startswith_ci(line->op, "face")) {
            int nverts = atoi(line->op + 4);
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if (nverts == 2 && arg_count >= 7) {
                bool ok_a = true;
                bool ok_b = true;
                bool ok_c = true;
                bool ok_v = true;
                uint8 color_index = (uint8)viewer_eval_expr_string(file, args[0], &ok_c);
                int16 vis_index = (int16)viewer_eval_expr_string(file, args[1], &ok_v);
                uint16 a = (uint16)viewer_eval_expr_string(file, args[arg_count - 2], &ok_a);
                uint16 b = (uint16)viewer_eval_expr_string(file, args[arg_count - 1], &ok_b);
                if (ok_a && ok_b && ok_c && ok_v) {
                    current_group = viewer_ensure_face_group(shape, current_group, viewer_attached_label(file, i));
                    viewer_append_line_face(shape, a, b, color_index, current_group);
                    shape->line_faces[shape->line_face_count - 1].vis_index = vis_index;
                }
            } else if (nverts >= 3 && nverts <= VIEWER_MAX_FACE_VERTS && arg_count >= nverts + 5) {
                int base = arg_count - nverts;
                ViewerPolyFace *face;
                bool color_ok = true;
                int v;
                current_group = viewer_ensure_face_group(shape, current_group, viewer_attached_label(file, i));
                VIEWER_ARRAY_GROW(shape->poly_faces, shape->poly_face_count, shape->poly_face_capacity, ViewerPolyFace);
                face = &shape->poly_faces[shape->poly_face_count++];
                memset(face, 0, sizeof(*face));
                face->count = (uint8)nverts;
                face->vis_index = (int16)viewer_eval_expr_string(file, args[1], &color_ok);
                face->group_index = (int16)current_group;
                face->color_index = (uint8)viewer_eval_expr_string(file, args[0], &color_ok);
                face->nx = (int16)viewer_eval_expr_string(file, args[2], &color_ok);
                face->ny = (int16)viewer_eval_expr_string(file, args[3], &color_ok);
                face->nz = (int16)viewer_eval_expr_string(file, args[4], &color_ok);
                for (v = 0; v < nverts; ++v) {
                    bool index_ok = true;
                    face->indices[v] = (uint16)viewer_eval_expr_string(file, args[base + v], &index_ok);
                    if (!index_ok) {
                        face->indices[v] = 0u;
                    }
                }
                if (current_group >= 0 && current_group < shape->face_group_count) {
                    ++shape->face_groups[current_group].poly_count;
                }
            }
            viewer_free_split_args(args, arg_count);
        } else if (viewer_stricmp(line->op, "aface3") == 0) {
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if (arg_count >= 8) {
                bool ok = true;
                uint16 v0 = (uint16)viewer_eval_expr_string(file, args[5], &ok);
                uint16 v1 = (uint16)viewer_eval_expr_string(file, args[6], &ok);
                uint16 v2 = (uint16)viewer_eval_expr_string(file, args[7], &ok);
                if (ok) {
                    current_group = viewer_ensure_face_group(shape, current_group, viewer_attached_label(file, i));
                    viewer_append_line_face(shape, v1, v2, 44u, current_group);
                    viewer_append_line_face(shape, v0, v1, 44u, current_group);
                    viewer_append_line_face(shape, v0, v2, 44u, current_group);
                }
            }
            viewer_free_split_args(args, arg_count);
        } else if (viewer_stricmp(line->op, "aface4") == 0) {
            int arg_count = 0;
            char **args = viewer_split_args(line->args, &arg_count);
            if (arg_count >= 9) {
                bool ok = true;
                uint16 v0 = (uint16)viewer_eval_expr_string(file, args[5], &ok);
                uint16 v1 = (uint16)viewer_eval_expr_string(file, args[6], &ok);
                uint16 v2 = (uint16)viewer_eval_expr_string(file, args[7], &ok);
                uint16 v3 = (uint16)viewer_eval_expr_string(file, args[8], &ok);
                if (ok) {
                    current_group = viewer_ensure_face_group(shape, current_group, viewer_attached_label(file, i));
                    viewer_append_line_face(shape, v0, v1, 44u, current_group);
                    viewer_append_line_face(shape, v1, v2, 44u, current_group);
                    viewer_append_line_face(shape, v2, v3, 44u, current_group);
                    viewer_append_line_face(shape, v3, v0, 44u, current_group);
                }
            }
            viewer_free_split_args(args, arg_count);
        }
    }
    viewer_resolve_bsp_nodes(shape);
    return true;
}

static void viewer_build_shape_vertex_normals(ViewerShape *shape) {
    int i;
    if (!shape || shape->frame_count <= 0 || shape->frames[0].vertex_count <= 0) {
        return;
    }
    shape->vertex_normal_count = shape->frames[0].vertex_count;
    shape->vertex_normals = (float *)calloc((size_t)shape->vertex_normal_count * 3u, sizeof(float));
    if (!shape->vertex_normals) {
        fprintf(stderr, "Viewer: out of memory building vertex normals\n");
        exit(1);
    }
    for (i = 0; i < shape->poly_face_count; ++i) {
        const ViewerPolyFace *face = &shape->poly_faces[i];
        float fx = (float)face->nx;
        float fy = (float)face->ny;
        float fz = (float)face->nz;
        float len = sqrtf(fx * fx + fy * fy + fz * fz);
        int v;
        if (face->count < 3 || len <= 0.0001f) {
            continue;
        }
        fx /= len;
        fy /= len;
        fz /= len;
        for (v = 0; v < (int)face->count; ++v) {
            uint16 vertex_index = face->indices[v];
            float *normal;
            if (vertex_index >= (uint16)shape->vertex_normal_count) {
                continue;
            }
            normal = &shape->vertex_normals[(size_t)vertex_index * 3u];
            normal[0] += fx;
            normal[1] += fy;
            normal[2] += fz;
        }
    }
    for (i = 0; i < shape->vertex_normal_count; ++i) {
        float *normal = &shape->vertex_normals[(size_t)i * 3u];
        float len = sqrtf(normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]);
        if (len > 0.0001f) {
            normal[0] /= len;
            normal[1] /= len;
            normal[2] /= len;
        }
    }
}

static GLuint viewer_get_vertex_color_shader(void) {
    if (!s_vertex_color_shader) {
        GLuint vert = GlBackend_CompileShader(s_vertex_color_vert_src, GL_VERTEX_SHADER);
        GLuint frag = GlBackend_CompileShader(s_vertex_color_frag_src, GL_FRAGMENT_SHADER);
        if (!vert || !frag) {
            if (vert) glDeleteShader(vert);
            if (frag) glDeleteShader(frag);
            return 0;
        }
        s_vertex_color_shader = GlBackend_LinkProgram(vert, frag);
        glDeleteShader(vert);
        glDeleteShader(frag);
    }
    return s_vertex_color_shader;
}

static void viewer_free_gpu_shape(ViewerGpuShape *gpu) {
    if (gpu->poly_vao) glDeleteVertexArrays(1, &gpu->poly_vao);
    if (gpu->poly_vbo) glDeleteBuffers(1, &gpu->poly_vbo);
    if (gpu->poly_color_vbo) glDeleteBuffers(1, &gpu->poly_color_vbo);
    if (gpu->line_vao) glDeleteVertexArrays(1, &gpu->line_vao);
    if (gpu->line_vbo) glDeleteBuffers(1, &gpu->line_vbo);
    free(gpu->poly_tri_start);
    free(gpu->poly_tri_count);
    memset(gpu, 0, sizeof(*gpu));
    gpu->uploaded_frame = -1;
}

static bool viewer_ensure_gpu_shape(ViewerShape *shape, int frame_index) {
    int i;
    int total_tris = 0;
    int total_lines = 0;
    float *poly_positions = NULL;
    float *line_positions = NULL;
    int poly_pos_index = 0;
    int line_pos_index = 0;
    ViewerFrame *frame;
    if (frame_index < 0 || frame_index >= shape->frame_count) {
        return false;
    }
    frame = &shape->frames[frame_index];

    if (!shape->gpu.ready) {
        glGenVertexArrays(1, &shape->gpu.poly_vao);
        glGenBuffers(1, &shape->gpu.poly_vbo);
        glGenBuffers(1, &shape->gpu.poly_color_vbo);
        glGenVertexArrays(1, &shape->gpu.line_vao);
        glGenBuffers(1, &shape->gpu.line_vbo);
        shape->gpu.poly_tri_start = (int *)malloc(sizeof(int) * (size_t)shape->poly_face_count);
        shape->gpu.poly_tri_count = (int *)malloc(sizeof(int) * (size_t)shape->poly_face_count);
        shape->gpu.uploaded_frame = -1;
        shape->gpu.ready = true;
    }
    if (shape->gpu.uploaded_frame == frame_index) {
        return true;
    }

    for (i = 0; i < shape->poly_face_count; ++i) {
        int tri_count = (int)shape->poly_faces[i].count - 2;
        if (tri_count < 0) {
            tri_count = 0;
        }
        shape->gpu.poly_tri_start[i] = total_tris;
        shape->gpu.poly_tri_count[i] = tri_count;
        total_tris += tri_count;
    }
    total_lines = shape->line_face_count;

    if (total_tris > 0) {
        poly_positions = (float *)malloc(sizeof(float) * (size_t)total_tris * 9u);
    }
    if (total_lines > 0) {
        line_positions = (float *)malloc(sizeof(float) * (size_t)total_lines * 6u);
    }
    if ((total_tris > 0 && !poly_positions) || (total_lines > 0 && !line_positions)) {
        free(poly_positions);
        free(line_positions);
        return false;
    }

    for (i = 0; i < shape->poly_face_count; ++i) {
        const ViewerPolyFace *face = &shape->poly_faces[i];
        int t;
        for (t = 0; t < (int)face->count - 2; ++t) {
            uint16 i0 = face->indices[0];
            uint16 i1 = face->indices[t + 1];
            uint16 i2 = face->indices[t + 2];
            ViewerVertex v0 = {0};
            ViewerVertex v1 = {0};
            ViewerVertex v2 = {0};
            if (i0 < frame->vertex_count) v0 = frame->vertices[i0];
            if (i1 < frame->vertex_count) v1 = frame->vertices[i1];
            if (i2 < frame->vertex_count) v2 = frame->vertices[i2];
            poly_positions[poly_pos_index++] = v0.x;
            poly_positions[poly_pos_index++] = v0.y;
            poly_positions[poly_pos_index++] = v0.z;
            poly_positions[poly_pos_index++] = v1.x;
            poly_positions[poly_pos_index++] = v1.y;
            poly_positions[poly_pos_index++] = v1.z;
            poly_positions[poly_pos_index++] = v2.x;
            poly_positions[poly_pos_index++] = v2.y;
            poly_positions[poly_pos_index++] = v2.z;
        }
    }

    for (i = 0; i < shape->line_face_count; ++i) {
        const ViewerLineFace *line = &shape->line_faces[i];
        ViewerVertex a = {0};
        ViewerVertex b = {0};
        if (line->a < frame->vertex_count) a = frame->vertices[line->a];
        if (line->b < frame->vertex_count) b = frame->vertices[line->b];
        line_positions[line_pos_index++] = a.x;
        line_positions[line_pos_index++] = a.y;
        line_positions[line_pos_index++] = a.z;
        line_positions[line_pos_index++] = b.x;
        line_positions[line_pos_index++] = b.y;
        line_positions[line_pos_index++] = b.z;
    }

    glBindVertexArray(shape->gpu.poly_vao);
    glBindBuffer(GL_ARRAY_BUFFER, shape->gpu.poly_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(float) * (size_t)poly_pos_index, poly_positions, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glBindBuffer(GL_ARRAY_BUFFER, shape->gpu.poly_color_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(float) * (size_t)(total_tris * 3) * 4u, NULL, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(1);

    glBindVertexArray(shape->gpu.line_vao);
    glBindBuffer(GL_ARRAY_BUFFER, shape->gpu.line_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(float) * (size_t)line_pos_index, line_positions, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);

    glBindVertexArray(0);

    shape->gpu.poly_vertex_count = total_tris * 3;
    shape->gpu.line_vertex_count = total_lines * 2;
    shape->gpu.uploaded_frame = frame_index;

    free(poly_positions);
    free(line_positions);
    return true;
}

static void viewer_append_shape(ViewerShape *shape) {
    VIEWER_ARRAY_GROW(s_shapes, s_shape_count, s_shape_capacity, ViewerShape);
    s_shapes[s_shape_count++] = *shape;
}

static void viewer_clear_skipped_shapes(void) {
    int i;
    for (i = 0; i < s_skipped_shape_count; ++i) {
        free((char *)s_skipped_shapes[i].file_path);
        free((char *)s_skipped_shapes[i].line_text);
        free((char *)s_skipped_shapes[i].reason);
    }
    free(s_skipped_shapes);
    s_skipped_shapes = NULL;
    s_skipped_shape_count = 0;
    s_skipped_shape_capacity = 0;
}

static void viewer_record_skipped_shape(const ViewerAsmFile *file, int line_index, const char *reason) {
    ViewerSkippedShapeInfo *info;
    const char *path = (file && file->path) ? file->path : "";
    const char *text = "";
    if (file && line_index >= 0 && line_index < file->line_count && file->lines[line_index].text) {
        text = file->lines[line_index].text;
    }
    VIEWER_ARRAY_GROW(s_skipped_shapes, s_skipped_shape_count, s_skipped_shape_capacity, ViewerSkippedShapeInfo);
    info = &s_skipped_shapes[s_skipped_shape_count++];
    info->file_path = viewer_strdup(path);
    info->line_text = viewer_strdup(text);
    info->reason = viewer_strdup(reason ? reason : "unknown");
    info->line_number = line_index + 1;
}

static char *viewer_make_shape_symbol_label(const char *candidate) {
    size_t len;
    if (!candidate || !*candidate || viewer_stricmp(candidate, "0") == 0) {
        return NULL;
    }
    len = strlen(candidate);
    if (len > 2 &&
        candidate[len - 2] == '_' &&
        (candidate[len - 1] == 'P' || candidate[len - 1] == 'p' ||
         candidate[len - 1] == 'F' || candidate[len - 1] == 'f')) {
        len -= 2;
    }
    if (len == 0) {
        return NULL;
    }
    return viewer_strndup_local(candidate, len);
}

static char *viewer_try_derive_shape_label(char **args, int arg_count) {
    if (arg_count > 0 && args[0] && viewer_stricmp(args[0], "0") != 0) {
        return viewer_make_shape_symbol_label(args[0]);
    }
    if (arg_count > 2 && args[2] && viewer_stricmp(args[2], "0") != 0) {
        return viewer_make_shape_symbol_label(args[2]);
    }
    return NULL;
}

static char *viewer_try_derive_shape_label_from_line(const ViewerAsmLine *line) {
    const char *p;
    const char *start;
    char *candidate;
    char *normalized;
    size_t len;
    if (!line || !line->text) {
        return NULL;
    }
    p = line->text;
    while (*p && !isspace((unsigned char)*p)) {
        ++p;
    }
    while (*p && isspace((unsigned char)*p)) {
        ++p;
    }
    start = p;
    while (*p && *p != ',' && !isspace((unsigned char)*p)) {
        ++p;
    }
    len = (size_t)(p - start);
    if (len == 0) {
        return NULL;
    }
    candidate = viewer_strndup_local(start, len);
    normalized = viewer_make_shape_symbol_label(candidate);
    free(candidate);
    return normalized;
}

static int viewer_material_frame_count(uint16 material) {
    uint8 source = (uint8)(material >> 8);
    if ((material & 0x8000u) != 0u && (material & 0x4000u) == 0u) {
        uint16 anim_id = (uint16)(material & 0x3FFFu);
        if (anim_id < (uint16)s_anim_table_count && s_anim_tables[anim_id].frame_count > 0) {
            return s_anim_tables[anim_id].frame_count;
        }
    }
    if (source == 62u || source == 63u) {
        return 1;
    }
    return 1;
}

static int viewer_color_table_frame_count(int color_table_id) {
    int i;
    int max_frames = 1;
    if (color_table_id < 0 || color_table_id >= s_color_table_count) {
        return 1;
    }
    for (i = 0; i < s_color_tables[color_table_id].material_count; ++i) {
        int count = viewer_material_frame_count(s_color_tables[color_table_id].materials[i]);
        if (count > max_frames) {
            max_frames = count;
        }
    }
    return max_frames;
}

static bool viewer_compute_shape_bounds(const ViewerShape *shape, float *out_center_xyz, float *out_radius) {
    int frame_index;
    bool found = false;
    float min_x = 0.0f;
    float min_y = 0.0f;
    float min_z = 0.0f;
    float max_x = 0.0f;
    float max_y = 0.0f;
    float max_z = 0.0f;

    if (!shape || shape->frame_count <= 0) {
        if (out_center_xyz) {
            out_center_xyz[0] = 0.0f;
            out_center_xyz[1] = 0.0f;
            out_center_xyz[2] = 0.0f;
        }
        if (out_radius) {
            *out_radius = 16.0f;
        }
        return false;
    }

    for (frame_index = 0; frame_index < shape->frame_count; ++frame_index) {
        const ViewerFrame *frame = &shape->frames[frame_index];
        int vertex_index;
        for (vertex_index = 0; vertex_index < frame->vertex_count; ++vertex_index) {
            const ViewerVertex *vertex = &frame->vertices[vertex_index];
            if (!found) {
                min_x = max_x = vertex->x;
                min_y = max_y = vertex->y;
                min_z = max_z = vertex->z;
                found = true;
            } else {
                if (vertex->x < min_x) min_x = vertex->x;
                if (vertex->x > max_x) max_x = vertex->x;
                if (vertex->y < min_y) min_y = vertex->y;
                if (vertex->y > max_y) max_y = vertex->y;
                if (vertex->z < min_z) min_z = vertex->z;
                if (vertex->z > max_z) max_z = vertex->z;
            }
        }
    }

    if (!found) {
        if (out_center_xyz) {
            out_center_xyz[0] = 0.0f;
            out_center_xyz[1] = 0.0f;
            out_center_xyz[2] = 0.0f;
        }
        if (out_radius) {
            *out_radius = 16.0f;
        }
        return false;
    }

    if (out_center_xyz) {
        float center_x = (min_x + max_x) * 0.5f;
        float center_y = (min_y + max_y) * 0.5f;
        float center_z = (min_z + max_z) * 0.5f;
        out_center_xyz[0] = center_x;
        out_center_xyz[1] = center_y;
        out_center_xyz[2] = center_z;
        if (out_radius) {
            float radius = 0.0f;
            for (frame_index = 0; frame_index < shape->frame_count; ++frame_index) {
                const ViewerFrame *frame = &shape->frames[frame_index];
                int vertex_index;
                for (vertex_index = 0; vertex_index < frame->vertex_count; ++vertex_index) {
                    const ViewerVertex *vertex = &frame->vertices[vertex_index];
                    float dx = vertex->x - center_x;
                    float dy = vertex->y - center_y;
                    float dz = vertex->z - center_z;
                    float dist = sqrtf(dx * dx + dy * dy + dz * dz);
                    if (dist > radius) {
                        radius = dist;
                    }
                }
            }
            *out_radius = radius > 1.0f ? radius : 1.0f;
        }
    } else if (out_radius) {
        float center_x = (min_x + max_x) * 0.5f;
        float center_y = (min_y + max_y) * 0.5f;
        float center_z = (min_z + max_z) * 0.5f;
        float radius = 0.0f;
        for (frame_index = 0; frame_index < shape->frame_count; ++frame_index) {
            const ViewerFrame *frame = &shape->frames[frame_index];
            int vertex_index;
            for (vertex_index = 0; vertex_index < frame->vertex_count; ++vertex_index) {
                const ViewerVertex *vertex = &frame->vertices[vertex_index];
                float dx = vertex->x - center_x;
                float dy = vertex->y - center_y;
                float dz = vertex->z - center_z;
                float dist = sqrtf(dx * dx + dy * dy + dz * dz);
                if (dist > radius) {
                    radius = dist;
                }
            }
        }
        *out_radius = radius > 1.0f ? radius : 1.0f;
    }

    return true;
}

static void viewer_try_parse_shape_header(ViewerAsmFile *file, int line_index) {
    ViewerAsmLine *line = &file->lines[line_index];
    const char *shape_label;
    char *derived_shape_label = NULL;
    ViewerShape shape;
    char **args;
    int arg_count;
    bool ok = true;
    if (!line->op || (viewer_stricmp(line->op, "shapehdr") != 0 && viewer_stricmp(line->op, "shapehdr_s") != 0)) {
        return;
    }
    ++s_shape_header_count;
    args = viewer_split_args(line->args, &arg_count);
    if (arg_count < 14) {
        viewer_record_skipped_shape(file, line_index, "short-shapehdr-args");
        viewer_free_split_args(args, arg_count);
        return;
    }
    shape_label = viewer_attached_label(file, line_index);
    if (!shape_label) {
        derived_shape_label = viewer_try_derive_shape_label(args, arg_count);
        if (!derived_shape_label) {
            derived_shape_label = viewer_try_derive_shape_label_from_line(line);
        }
        shape_label = derived_shape_label;
    }
    if (!shape_label) {
        viewer_record_skipped_shape(file, line_index, "missing-attached-label");
        viewer_free_split_args(args, arg_count);
        return;
    }
    memset(&shape, 0, sizeof(shape));
    memset(&shape.gpu, 0, sizeof(shape.gpu));
    shape.gpu.uploaded_frame = -1;
    shape.bsp_root_index = -1;

    shape.info.label = viewer_strdup(shape_label);
    shape.info.points_label = viewer_strdup(args[0]);
    shape.info.faces_label = viewer_strdup(args[2]);
    shape.info.color_table_label = viewer_strdup(args[13]);
    shape.info.display_name = viewer_strdup(args[arg_count - 1]);
    shape.info.shift = viewer_eval_expr_string(file, args[7], &ok);
    if (!ok) {
        shape.info.shift = 0;
    }
    if (viewer_stricmp(shape.info.color_table_label, "0") == 0) {
        shape.color_table_id = -1;
    } else {
        shape.color_table_id = viewer_parse_color_table(shape.info.color_table_label);
        if (shape.color_table_id >= 0 && shape.color_table_id < s_color_table_count) {
            shape.info.has_textured_materials = s_color_tables[shape.color_table_id].has_textures;
        }
    }
    if (viewer_stricmp(shape.info.points_label, "0") != 0 && viewer_stricmp(shape.info.faces_label, "0") != 0) {
        viewer_build_shape_frames(file, shape.info.points_label, shape.info.shift, &shape.frames, &shape.frame_count);
        viewer_build_shape_faces(file, shape.info.faces_label, &shape);
        viewer_build_shape_vertex_normals(&shape);
        if (shape.frame_count > 0) {
            shape.info.vertex_count = shape.frames[0].vertex_count;
            shape.info.frame_count = shape.frame_count;
        } else {
            shape.info.frame_count = 1;
        }
        shape.info.color_frame_count = viewer_color_table_frame_count(shape.color_table_id);
        shape.info.poly_face_count = shape.poly_face_count;
        shape.info.line_face_count = shape.line_face_count;
    } else {
        shape.info.frame_count = 1;
        shape.info.color_frame_count = 1;
    }
    viewer_append_shape(&shape);
    free(derived_shape_label);
    viewer_free_split_args(args, arg_count);
}

static bool viewer_load_source_files(void) {
    static const char *shape_paths[] = {
        "reference/ultrastarfox/SF/SHAPES/SHAPES.ASM",
        "reference/ultrastarfox/SF/SHAPES/SHAPES2.ASM",
        "reference/ultrastarfox/SF/SHAPES/SHAPES3.ASM",
        "reference/ultrastarfox/SF/SHAPES/SHAPES4.ASM",
        "reference/ultrastarfox/SF/SHAPES/SHAPES5.ASM",
        "reference/ultrastarfox/SF/SHAPES/SHAPES6.ASM",
        "reference/ultrastarfox/SF/SHAPES/USHAPES.ASM",
        "reference/ultrastarfox/SF/SHAPES/KSHAPES.ASM",
        "reference/ultrastarfox/SF/SHAPES/PSHAPES.ASM",
    };
    size_t i;
    s_shape_file_count = 0;
    for (i = 0; i < ARRAY_SIZE(shape_paths); ++i) {
        if (!viewer_load_asm_file(&s_shape_files[s_shape_file_count], shape_paths[i])) {
            return false;
        }
        viewer_collect_symbols(&s_shape_files[s_shape_file_count]);
        ++s_shape_file_count;
    }
    if (!viewer_load_asm_file(&s_coltabs_file, "reference/ultrastarfox/SF/ASM/COLTABS.ASM")) {
        return false;
    }
    if (!viewer_load_asm_file(&s_coltab_file, "reference/ultrastarfox/SF/ASM/COLTAB.ASM")) {
        return false;
    }
    if (!viewer_load_asm_file(&s_light_file, "reference/ultrastarfox/SF/ASM/LIGHT.ASM")) {
        return false;
    }
    viewer_collect_symbols(&s_coltabs_file);
    viewer_collect_symbols(&s_coltab_file);
    viewer_collect_symbols(&s_light_file);
    return true;
}

bool ViewerCatalog_LoadFromAsm(void) {
    int file_index;
    int i;
    if (s_shape_count > 0) {
        return true;
    }
    if (!viewer_load_palette_file("reference/ultrastarfox/SF/DATA/COL/NIGHT.COL", s_norm_palette) ||
        !viewer_load_palette_file("reference/ultrastarfox/SF/DATA/COL/RED.COL", s_red_palette) ||
        !viewer_load_palette_file("reference/ultrastarfox/SF/DATA/COL/BLUE.COL", s_blue_palette)) {
        return false;
    }
    if (!viewer_load_source_files()) {
        return false;
    }

    s_shape_header_count = 0;
    viewer_clear_skipped_shapes();

    s_depth_norm = viewer_parse_depth_table("night1");
    s_depth_red = viewer_parse_depth_table("red1");
    s_depth_blue = viewer_parse_depth_table("blue1");
    viewer_parse_light_tables();

    for (file_index = 0; file_index < s_shape_file_count; ++file_index) {
        ViewerAsmFile *file = &s_shape_files[file_index];
        for (i = 0; i < file->line_count; ++i) {
            viewer_try_parse_shape_header(file, i);
        }
    }

    printf("Viewer: loaded %d shapes from %d headers (%d skipped), %d color tables, %d anim tables\n",
           s_shape_count, s_shape_header_count, s_skipped_shape_count, s_color_table_count, s_anim_table_count);
    return s_shape_count > 0;
}

void ViewerCatalog_Unload(void) {
    int i;
    int j;
    for (i = 0; i < s_shape_count; ++i) {
        ViewerShape *shape = &s_shapes[i];
        viewer_free_gpu_shape(&shape->gpu);
        for (j = 0; j < shape->frame_count; ++j) {
            free(shape->frames[j].vertices);
        }
        free((char *)shape->info.label);
        free((char *)shape->info.display_name);
        free((char *)shape->info.points_label);
        free((char *)shape->info.faces_label);
        free((char *)shape->info.color_table_label);
        free(shape->frames);
        free(shape->poly_faces);
        free(shape->line_faces);
        free(shape->viz_entries);
        for (j = 0; j < shape->face_group_count; ++j) {
            free(shape->face_groups[j].label);
        }
        for (j = 0; j < shape->bsp_node_count; ++j) {
            free(shape->bsp_nodes[j].label);
            free(shape->bsp_nodes[j].face_group_label);
            free(shape->bsp_nodes[j].right_label);
        }
        free(shape->face_groups);
        free(shape->bsp_nodes);
        free(shape->vertex_normals);
    }
    free(s_shapes);
    s_shapes = NULL;
    s_shape_count = 0;
    s_shape_capacity = 0;
    s_shape_header_count = 0;
    viewer_clear_skipped_shapes();

    for (i = 0; i < s_color_table_count; ++i) {
        free(s_color_tables[i].label);
        free(s_color_tables[i].materials);
    }
    free(s_color_tables);
    s_color_tables = NULL;
    s_color_table_count = 0;
    s_color_table_capacity = 0;

    for (i = 0; i < s_anim_table_count; ++i) {
        free(s_anim_tables[i].label);
        free(s_anim_tables[i].frames);
    }
    free(s_anim_tables);
    s_anim_tables = NULL;
    s_anim_table_count = 0;
    s_anim_table_capacity = 0;

    for (i = 0; i < s_depth_table_count; ++i) {
        free(s_depth_tables[i].label);
        free(s_depth_tables[i].pairs);
    }
    free(s_depth_tables);
    s_depth_tables = NULL;
    s_depth_table_count = 0;
    s_depth_table_capacity = 0;

    for (i = 0; i < s_shape_file_count; ++i) {
        ViewerAsmFile *file = &s_shape_files[i];
        for (j = 0; j < file->line_count; ++j) {
            free(file->lines[j].text);
            free(file->lines[j].label);
            free(file->lines[j].op);
            free(file->lines[j].args);
        }
        for (j = 0; j < file->symbol_count; ++j) {
            free(file->symbols[j].name);
            free(file->symbols[j].expr);
        }
        free(file->lines);
        free(file->symbols);
        memset(file, 0, sizeof(*file));
    }
    s_shape_file_count = 0;

    for (i = 0; i < s_coltabs_file.line_count; ++i) {
        free(s_coltabs_file.lines[i].text);
        free(s_coltabs_file.lines[i].label);
        free(s_coltabs_file.lines[i].op);
        free(s_coltabs_file.lines[i].args);
    }
    for (i = 0; i < s_coltabs_file.symbol_count; ++i) {
        free(s_coltabs_file.symbols[i].name);
        free(s_coltabs_file.symbols[i].expr);
    }
    free(s_coltabs_file.lines);
    free(s_coltabs_file.symbols);
    memset(&s_coltabs_file, 0, sizeof(s_coltabs_file));

    for (i = 0; i < s_coltab_file.line_count; ++i) {
        free(s_coltab_file.lines[i].text);
        free(s_coltab_file.lines[i].label);
        free(s_coltab_file.lines[i].op);
        free(s_coltab_file.lines[i].args);
    }
    for (i = 0; i < s_coltab_file.symbol_count; ++i) {
        free(s_coltab_file.symbols[i].name);
        free(s_coltab_file.symbols[i].expr);
    }
    free(s_coltab_file.lines);
    free(s_coltab_file.symbols);
    memset(&s_coltab_file, 0, sizeof(s_coltab_file));

    viewer_free_asm_file(&s_light_file);
    if (s_vertex_color_shader) {
        glDeleteProgram(s_vertex_color_shader);
        s_vertex_color_shader = 0;
    }
    s_light_tables_ready = false;
    memset(s_light_pairs, 0, sizeof(s_light_pairs));
    s_shade_table_index = 0;
}

int ViewerCatalog_GetShapeCount(void) {
    return s_shape_count;
}

int ViewerCatalog_GetShapeHeaderCount(void) {
    return s_shape_header_count;
}

const ViewerShapeInfo *ViewerCatalog_GetShapeInfo(int index) {
    if (index < 0 || index >= s_shape_count) {
        return NULL;
    }
    return &s_shapes[index].info;
}

int ViewerCatalog_FindShapeByLabel(const char *label) {
    int i;
    if (!label || !*label) {
        return -1;
    }
    for (i = 0; i < s_shape_count; ++i) {
        if (s_shapes[i].info.label && viewer_stricmp(s_shapes[i].info.label, label) == 0) {
            return i;
        }
        if (s_shapes[i].info.display_name && viewer_stricmp(s_shapes[i].info.display_name, label) == 0) {
            return i;
        }
    }
    return -1;
}

int ViewerCatalog_GetShapeFrameCount(int index) {
    if (index < 0 || index >= s_shape_count) {
        return 1;
    }
    return s_shapes[index].frame_count > 0 ? s_shapes[index].frame_count : 1;
}

int ViewerCatalog_GetShapeColorFrameCount(int index) {
    if (index < 0 || index >= s_shape_count) {
        return 1;
    }
    return s_shapes[index].info.color_frame_count > 0 ? s_shapes[index].info.color_frame_count : 1;
}

bool ViewerCatalog_GetShapeBounds(int index, float *out_center_xyz, float *out_radius) {
    if (index < 0 || index >= s_shape_count) {
        if (out_center_xyz) {
            out_center_xyz[0] = 0.0f;
            out_center_xyz[1] = 0.0f;
            out_center_xyz[2] = 0.0f;
        }
        if (out_radius) {
            *out_radius = 16.0f;
        }
        return false;
    }
    return viewer_compute_shape_bounds(&s_shapes[index], out_center_xyz, out_radius);
}

int ViewerCatalog_GetSkippedShapeCount(void) {
    return s_skipped_shape_count;
}

const ViewerSkippedShapeInfo *ViewerCatalog_GetSkippedShapeInfo(int index) {
    if (index < 0 || index >= s_skipped_shape_count) {
        return NULL;
    }
    return &s_skipped_shapes[index];
}

void ViewerCatalog_SetPalette(ViewerPaletteKind palette) {
    if (palette >= 0 && palette < VIEWER_PALETTE_COUNT) {
        s_palette_kind = palette;
    }
}

ViewerPaletteKind ViewerCatalog_GetPalette(void) {
    return s_palette_kind;
}

void ViewerCatalog_NextPalette(int delta) {
    int value = (int)s_palette_kind + delta;
    while (value < 0) {
        value += VIEWER_PALETTE_COUNT;
    }
    value %= VIEWER_PALETTE_COUNT;
    s_palette_kind = (ViewerPaletteKind)value;
}

const char *ViewerCatalog_GetPaletteName(ViewerPaletteKind palette) {
    switch (palette) {
    case VIEWER_PALETTE_RED:
        return "red";
    case VIEWER_PALETTE_BLUE:
        return "blue";
    case VIEWER_PALETTE_NORM:
    default:
        return "night";
    }
}

void ViewerCatalog_SetShadeTable(int shade_table) {
    if (shade_table < 0) {
        shade_table = 0;
    }
    if (shade_table > 3) {
        shade_table = 3;
    }
    s_shade_table_index = shade_table;
}

int ViewerCatalog_GetShadeTable(void) {
    return s_shade_table_index;
}

void ViewerCatalog_NextShadeTable(int delta) {
    int value = s_shade_table_index + delta;
    while (value < 0) {
        value += 4;
    }
    value %= 4;
    s_shade_table_index = value;
}

static bool viewer_build_poly_colors(const ViewerShape *shape, int col_frame, float *poly_colors) {
    int i;
    int color_pos_index = 0;
    if (!shape || !poly_colors) {
        return false;
    }
    for (i = 0; i < shape->poly_face_count; ++i) {
        const ViewerPolyFace *face = &shape->poly_faces[i];
        uint16 material = 0;
        bool is_smooth = false;
        int t;
        if (face->count < 3) {
            continue;
        }
        if (viewer_get_face_material(shape, face->color_index, col_frame, &material)) {
            is_smooth = viewer_material_is_smooth(material);
        }
        for (t = 0; t < (int)face->count - 2; ++t) {
            uint16 tri_indices[3];
            int v;
            tri_indices[0] = face->indices[0];
            tri_indices[1] = face->indices[t + 1];
            tri_indices[2] = face->indices[t + 2];
            for (v = 0; v < 3; ++v) {
                float color[4];
                int shade_index;
                if (is_smooth &&
                    shape->vertex_normals &&
                    tri_indices[v] < (uint16)shape->vertex_normal_count) {
                    const float *normal = &shape->vertex_normals[(size_t)tri_indices[v] * 3u];
                    shade_index = viewer_compute_shade_index_vec3(normal[0], normal[1], normal[2]);
                } else {
                    shade_index = viewer_compute_shade_index(face->nx, face->ny, face->nz);
                }
                viewer_resolve_face_color(shape, face->color_index, col_frame, shade_index, color);
                poly_colors[color_pos_index++] = color[0];
                poly_colors[color_pos_index++] = color[1];
                poly_colors[color_pos_index++] = color[2];
                poly_colors[color_pos_index++] = color[3];
            }
        }
    }
    return true;
}

static void viewer_mul_mat4_vec4(const float *m, float x, float y, float z, float w, float out[4]) {
    out[0] = m[0] * x + m[4] * y + m[8]  * z + m[12] * w;
    out[1] = m[1] * x + m[5] * y + m[9]  * z + m[13] * w;
    out[2] = m[2] * x + m[6] * y + m[10] * z + m[14] * w;
    out[3] = m[3] * x + m[7] * y + m[11] * z + m[15] * w;
}

static bool viewer_compute_vis_flags(const ViewerShape *shape,
                                     const ViewerFrame *frame,
                                     const float *model_matrix,
                                     const float *view_matrix,
                                     const float *proj_matrix,
                                     uint8 *vis_flags) {
    int i;
    float model_view[16];
    float mvp[16];
    if (!shape || !frame || !vis_flags) {
        return false;
    }
    if (shape->viz_count <= 0 || !model_matrix || !view_matrix || !proj_matrix) {
        return false;
    }
    Transform_Multiply(model_view, view_matrix, model_matrix);
    Transform_Multiply(mvp, proj_matrix, model_view);
    for (i = 0; i < shape->viz_count; ++i) {
        const ViewerVizEntry *viz = &shape->viz_entries[i];
        float p1[4];
        float p2[4];
        float p3[4];
        float x1, y1, x2, y2, x3, y3;
        float area2;
        if (viz->p1 >= (uint16)frame->vertex_count ||
            viz->p2 >= (uint16)frame->vertex_count ||
            viz->p3 >= (uint16)frame->vertex_count) {
            vis_flags[i] = 1u;
            continue;
        }
        viewer_mul_mat4_vec4(mvp,
                             frame->vertices[viz->p1].x, frame->vertices[viz->p1].y, frame->vertices[viz->p1].z, 1.0f, p1);
        viewer_mul_mat4_vec4(mvp,
                             frame->vertices[viz->p2].x, frame->vertices[viz->p2].y, frame->vertices[viz->p2].z, 1.0f, p2);
        viewer_mul_mat4_vec4(mvp,
                             frame->vertices[viz->p3].x, frame->vertices[viz->p3].y, frame->vertices[viz->p3].z, 1.0f, p3);
        if (fabsf(p1[3]) <= 0.0001f || fabsf(p2[3]) <= 0.0001f || fabsf(p3[3]) <= 0.0001f) {
            vis_flags[i] = 1u;
            continue;
        }
        x1 = p1[0] / p1[3];
        y1 = p1[1] / p1[3];
        x2 = p2[0] / p2[3];
        y2 = p2[1] / p2[3];
        x3 = p3[0] / p3[3];
        y3 = p3[1] / p3[3];
        area2 = (x2 - x1) * (y3 - y1) - (y2 - y1) * (x3 - x1);
        vis_flags[i] = (area2 >= 0.0f) ? 1u : 0u;
    }
    return true;
}

static bool viewer_face_is_visible(int16 vis_index, const uint8 *vis_flags, int vis_count) {
    if (vis_index < 0) {
        return true;
    }
    if (!vis_flags || vis_index >= vis_count) {
        return true;
    }
    return vis_flags[vis_index] != 0u;
}

static void viewer_collect_bsp_group_order(const ViewerShape *shape,
                                           int node_index,
                                           const uint8 *vis_flags,
                                           uint8 *group_seen,
                                           uint8 *node_seen,
                                           int *group_order,
                                           int *io_count) {
    const ViewerBspNode *node;
    if (!shape || !group_seen || !node_seen || !group_order || !io_count) {
        return;
    }
    if (node_index < 0 || node_index >= shape->bsp_node_count) {
        return;
    }
    if (node_seen[node_index] != 0u) {
        return;
    }
    node_seen[node_index] = 1u;
    node = &shape->bsp_nodes[node_index];
    if (viewer_face_is_visible(node->vis_index, vis_flags, shape->viz_count) &&
        node->face_group_index >= 0 &&
        node->face_group_index < shape->face_group_count &&
        group_seen[node->face_group_index] == 0u) {
        group_seen[node->face_group_index] = 1u;
        group_order[(*io_count)++] = node->face_group_index;
    }
    if (node->right_node_index >= 0) {
        viewer_collect_bsp_group_order(shape, node->right_node_index, vis_flags,
                                       group_seen, node_seen, group_order, io_count);
    }
}

void ViewerCatalog_RenderShape(int index, int anim_frame, int col_frame,
                               const float *model_matrix,
                               const float *view_matrix,
                               const float *proj_matrix) {
    ViewerShape *shape;
    ViewerFrame *frame;
    GLuint poly_shader = 0;
    uint8 *vis_flags = NULL;
    uint8 *group_seen = NULL;
    uint8 *node_seen = NULL;
    int *group_order = NULL;
    int group_order_count = 0;
    int frame_index;
    int i;
    if (index < 0 || index >= s_shape_count) {
        return;
    }
    shape = &s_shapes[index];
    frame_index = 0;
    if (shape->frame_count > 0) {
        frame_index = anim_frame % shape->frame_count;
        if (frame_index < 0) {
            frame_index += shape->frame_count;
        }
    }
    if (!viewer_ensure_gpu_shape(shape, frame_index)) {
        return;
    }
    frame = &shape->frames[frame_index];
    if (shape->viz_count > 0) {
        vis_flags = (uint8 *)malloc((size_t)shape->viz_count);
        if (vis_flags) {
            memset(vis_flags, 1, (size_t)shape->viz_count);
            viewer_compute_vis_flags(shape, frame, model_matrix, view_matrix, proj_matrix, vis_flags);
        }
    }
    if (shape->bsp_root_index >= 0 && shape->face_group_count > 0) {
        group_seen = (uint8 *)calloc((size_t)shape->face_group_count, 1u);
        node_seen = (uint8 *)calloc((size_t)shape->bsp_node_count, 1u);
        group_order = (int *)malloc(sizeof(int) * (size_t)shape->face_group_count);
        if (group_seen && node_seen && group_order) {
            viewer_collect_bsp_group_order(shape, shape->bsp_root_index, vis_flags,
                                           group_seen, node_seen, group_order, &group_order_count);
        } else {
            free(group_seen);
            free(node_seen);
            free(group_order);
            group_seen = NULL;
            node_seen = NULL;
            group_order = NULL;
        }
    }

    poly_shader = viewer_get_vertex_color_shader();
    if (shape->gpu.poly_vertex_count > 0 && poly_shader) {
        float *poly_colors = (float *)malloc(sizeof(float) * (size_t)shape->gpu.poly_vertex_count * 4u);
        if (poly_colors) {
            viewer_build_poly_colors(shape, col_frame, poly_colors);
            glUseProgram(poly_shader);
            if (model_matrix) {
                GlBackend_SetMat4(poly_shader, "uModel", model_matrix);
            }
            if (view_matrix) {
                GlBackend_SetMat4(poly_shader, "uView", view_matrix);
            }
            if (proj_matrix) {
                GlBackend_SetMat4(poly_shader, "uProj", proj_matrix);
            }
            glBindVertexArray(shape->gpu.poly_vao);
            glBindBuffer(GL_ARRAY_BUFFER, shape->gpu.poly_color_vbo);
            glBufferSubData(GL_ARRAY_BUFFER, 0,
                            sizeof(float) * (size_t)shape->gpu.poly_vertex_count * 4u,
                            poly_colors);
            if (group_order_count > 0) {
                int order_index;
                for (order_index = 0; order_index < group_order_count; ++order_index) {
                    const ViewerFaceGroup *group = &shape->face_groups[group_order[order_index]];
                    for (i = group->poly_start; i < group->poly_start + group->poly_count; ++i) {
                        int tri_count = shape->gpu.poly_tri_count[i];
                        if (tri_count <= 0) {
                            continue;
                        }
                        if (!viewer_face_is_visible(shape->poly_faces[i].vis_index, vis_flags, shape->viz_count)) {
                            continue;
                        }
                        glDrawArrays(GL_TRIANGLES, shape->gpu.poly_tri_start[i] * 3, tri_count * 3);
                    }
                }
            } else {
                for (i = 0; i < shape->poly_face_count; ++i) {
                    int tri_count = shape->gpu.poly_tri_count[i];
                    if (tri_count <= 0) {
                        continue;
                    }
                    if (!viewer_face_is_visible(shape->poly_faces[i].vis_index, vis_flags, shape->viz_count)) {
                        continue;
                    }
                    glDrawArrays(GL_TRIANGLES, shape->gpu.poly_tri_start[i] * 3, tri_count * 3);
                }
            }
            glBindVertexArray(0);
            free(poly_colors);
        }
    }

    if (shape->gpu.line_vertex_count > 0) {
        glUseProgram(g_flat_shader);
        if (model_matrix) {
            GlBackend_SetMat4(g_flat_shader, "uModel", model_matrix);
        }
        if (view_matrix) {
            GlBackend_SetMat4(g_flat_shader, "uView", view_matrix);
        }
        if (proj_matrix) {
            GlBackend_SetMat4(g_flat_shader, "uProj", proj_matrix);
        }
        glLineWidth(1.5f);
        glBindVertexArray(shape->gpu.line_vao);
        if (group_order_count > 0) {
            int order_index;
            for (order_index = 0; order_index < group_order_count; ++order_index) {
                const ViewerFaceGroup *group = &shape->face_groups[group_order[order_index]];
                for (i = group->line_start; i < group->line_start + group->line_count; ++i) {
                    float color[4];
                    if (!viewer_face_is_visible(shape->line_faces[i].vis_index, vis_flags, shape->viz_count)) {
                        continue;
                    }
                    viewer_resolve_face_color(shape, shape->line_faces[i].color_index, col_frame, 9, color);
                    GlBackend_SetVec4(g_flat_shader, "uColor", color[0], color[1], color[2], color[3]);
                    glDrawArrays(GL_LINES, i * 2, 2);
                }
            }
        } else {
            for (i = 0; i < shape->line_face_count; ++i) {
                float color[4];
                if (!viewer_face_is_visible(shape->line_faces[i].vis_index, vis_flags, shape->viz_count)) {
                    continue;
                }
                viewer_resolve_face_color(shape, shape->line_faces[i].color_index, col_frame, 9, color);
                GlBackend_SetVec4(g_flat_shader, "uColor", color[0], color[1], color[2], color[3]);
                glDrawArrays(GL_LINES, i * 2, 2);
            }
        }
        glBindVertexArray(0);
    }
    free(group_seen);
    free(node_seen);
    free(group_order);
    free(vis_flags);
}
