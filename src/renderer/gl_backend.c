#include "gl_backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

GLuint g_flat_shader = 0;
GLuint g_hud_shader = 0;

// Embedded flat shader — no external files needed for basic rendering
static const char *flat_vert_src =
    "#version 330 core\n"
    "layout(location = 0) in vec3 aPos;\n"
    "uniform mat4 uModel;\n"
    "uniform mat4 uView;\n"
    "uniform mat4 uProj;\n"
    "void main() {\n"
    "    gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);\n"
    "}\n";

static const char *flat_frag_src =
    "#version 330 core\n"
    "uniform vec4 uColor;\n"
    "out vec4 FragColor;\n"
    "void main() {\n"
    "    FragColor = uColor;\n"
    "}\n";

// HUD shader (2D overlay)
static const char *hud_vert_src =
    "#version 330 core\n"
    "layout(location = 0) in vec2 aPos;\n"
    "layout(location = 1) in vec2 aTexCoord;\n"
    "uniform mat4 uProj;\n"
    "out vec2 vTexCoord;\n"
    "void main() {\n"
    "    gl_Position = uProj * vec4(aPos, 0.0, 1.0);\n"
    "    vTexCoord = aTexCoord;\n"
    "}\n";

static const char *hud_frag_src =
    "#version 330 core\n"
    "uniform vec4 uColor;\n"
    "in vec2 vTexCoord;\n"
    "out vec4 FragColor;\n"
    "void main() {\n"
    "    FragColor = uColor;\n"
    "}\n";

static char *LoadFile(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc(len + 1);
    if (buf) {
        fread(buf, 1, len, f);
        buf[len] = '\0';
    }
    fclose(f);
    return buf;
}

GLuint GlBackend_CompileShader(const char *source, GLenum type) {
    GLuint shader = glCreateShader(type);
    glShaderSource(shader, 1, &source, NULL);
    glCompileShader(shader);

    GLint success;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &success);
    if (!success) {
        char log[512];
        glGetShaderInfoLog(shader, sizeof(log), NULL, log);
        fprintf(stderr, "Shader compile error: %s\n", log);
        glDeleteShader(shader);
        return 0;
    }
    return shader;
}

GLuint GlBackend_LinkProgram(GLuint vert, GLuint frag) {
    GLuint program = glCreateProgram();
    glAttachShader(program, vert);
    glAttachShader(program, frag);
    glLinkProgram(program);

    GLint success;
    glGetProgramiv(program, GL_LINK_STATUS, &success);
    if (!success) {
        char log[512];
        glGetProgramInfoLog(program, sizeof(log), NULL, log);
        fprintf(stderr, "Shader link error: %s\n", log);
        glDeleteProgram(program);
        return 0;
    }
    return program;
}

GLuint GlBackend_LoadShader(const char *vert_path, const char *frag_path) {
    char *vert_src = LoadFile(vert_path);
    char *frag_src = LoadFile(frag_path);
    if (!vert_src || !frag_src) {
        fprintf(stderr, "Failed to load shader: %s / %s\n", vert_path, frag_path);
        free(vert_src);
        free(frag_src);
        return 0;
    }

    GLuint vert = GlBackend_CompileShader(vert_src, GL_VERTEX_SHADER);
    GLuint frag = GlBackend_CompileShader(frag_src, GL_FRAGMENT_SHADER);
    free(vert_src);
    free(frag_src);

    if (!vert || !frag) {
        if (vert) glDeleteShader(vert);
        if (frag) glDeleteShader(frag);
        return 0;
    }

    GLuint program = GlBackend_LinkProgram(vert, frag);
    glDeleteShader(vert);
    glDeleteShader(frag);
    return program;
}

static GLuint BuildEmbeddedShader(const char *vert_src, const char *frag_src) {
    GLuint vert = GlBackend_CompileShader(vert_src, GL_VERTEX_SHADER);
    GLuint frag = GlBackend_CompileShader(frag_src, GL_FRAGMENT_SHADER);
    if (!vert || !frag) {
        if (vert) glDeleteShader(vert);
        if (frag) glDeleteShader(frag);
        return 0;
    }
    GLuint program = GlBackend_LinkProgram(vert, frag);
    glDeleteShader(vert);
    glDeleteShader(frag);
    return program;
}

void GlBackend_Init(void) {
    g_flat_shader = BuildEmbeddedShader(flat_vert_src, flat_frag_src);
    g_hud_shader = BuildEmbeddedShader(hud_vert_src, hud_frag_src);

    if (!g_flat_shader) fprintf(stderr, "Failed to build flat shader\n");
    if (!g_hud_shader) fprintf(stderr, "Failed to build HUD shader\n");
}

void GlBackend_Shutdown(void) {
    if (g_flat_shader) glDeleteProgram(g_flat_shader);
    if (g_hud_shader) glDeleteProgram(g_hud_shader);
    g_flat_shader = 0;
    g_hud_shader = 0;
}

void GlBackend_SetMat4(GLuint program, const char *name, const float *mat) {
    GLint loc = glGetUniformLocation(program, name);
    if (loc >= 0) glUniformMatrix4fv(loc, 1, GL_FALSE, mat);
}

void GlBackend_SetVec3(GLuint program, const char *name, float x, float y, float z) {
    GLint loc = glGetUniformLocation(program, name);
    if (loc >= 0) glUniform3f(loc, x, y, z);
}

void GlBackend_SetVec4(GLuint program, const char *name, float x, float y, float z, float w) {
    GLint loc = glGetUniformLocation(program, name);
    if (loc >= 0) glUniform4f(loc, x, y, z, w);
}

void GlBackend_SetFloat(GLuint program, const char *name, float val) {
    GLint loc = glGetUniformLocation(program, name);
    if (loc >= 0) glUniform1f(loc, val);
}
