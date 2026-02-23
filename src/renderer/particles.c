#include "../types.h"
#include "gl_backend.h"
#include <glad/glad.h>

// Particle system stub — explosions, engine trails, laser impacts
// Will be populated when explosion/effect strategy code is converted

void Particles_Init(void) {
    // TODO: create particle VAO/VBO
}

void Particles_Shutdown(void) {
    // TODO: cleanup
}

void Particles_Update(float dt) {
    (void)dt;
    // TODO: update particle positions, lifetimes
}

void Particles_Render(void) {
    // TODO: render active particles
}
