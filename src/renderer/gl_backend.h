#ifndef STARFOX_GL_BACKEND_H
#define STARFOX_GL_BACKEND_H

#include "../types.h"
#include <glad/glad.h>

// Shader program for flat-shaded polygons
extern GLuint g_flat_shader;

// Shader program for HUD elements
extern GLuint g_hud_shader;

void GlBackend_Init(void);
void GlBackend_Shutdown(void);

// Shader utilities
GLuint GlBackend_LoadShader(const char *vert_path, const char *frag_path);
GLuint GlBackend_CompileShader(const char *source, GLenum type);
GLuint GlBackend_LinkProgram(GLuint vert, GLuint frag);

// Set uniform helpers
void GlBackend_SetMat4(GLuint program, const char *name, const float *mat);
void GlBackend_SetVec3(GLuint program, const char *name, float x, float y, float z);
void GlBackend_SetVec4(GLuint program, const char *name, float x, float y, float z, float w);
void GlBackend_SetFloat(GLuint program, const char *name, float val);

#endif // STARFOX_GL_BACKEND_H
