cmake_minimum_required(VERSION 3.16)
project(starfox-hd C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# Find dependencies
find_package(PkgConfig REQUIRED)
pkg_check_modules(SDL2 REQUIRED sdl2)

find_package(OpenGL REQUIRED)

# Glad (vendored OpenGL loader)
add_library(glad STATIC src/glad/src/glad.c)
target_include_directories(glad PUBLIC src/glad/include)

# Main executable
add_executable(starfox-hd
    src/main.c
    src/config.c
    src/sf_rtl.c
    src/snes/ppu_stub.c
    src/snes/apu.c
    src/renderer/renderer.c
    src/renderer/gl_backend.c
    src/renderer/shapes.c
    src/renderer/draw_list.c
    src/renderer/transform.c
    src/renderer/polygon.c
    src/renderer/hud.c
    src/renderer/particles.c
    src/audio/audio.c
    src/audio/spc_player.c
    src/game/boot.c
    src/game/nmi.c
    src/game/obj.c
    src/game/game.c
    src/game/coldet.c
    src/game/draw.c
    src/game/sound.c
    src/game/bgs.c
    src/game/windows.c
    src/game/strings.c
    src/game/world.c
    src/game/planets.c
    src/strat/strat_common.c
    src/strat/strat_player.c
    src/strat/strat_ground.c
    src/strat/strat_enemy.c
    src/path/paths.c
    src/map/map_exec.c
    src/map/levels.c
)

target_include_directories(starfox-hd PRIVATE
    src
    ${SDL2_INCLUDE_DIRS}
    ${OPENGL_INCLUDE_DIR}
)

target_link_libraries(starfox-hd PRIVATE
    ${SDL2_LIBRARIES}
    OpenGL::GL
    glad
    m
)

target_compile_options(starfox-hd PRIVATE
    ${SDL2_CFLAGS_OTHER}
    -Wall -Wextra -Wno-unused-parameter
)

# Copy assets
file(COPY ${CMAKE_SOURCE_DIR}/starfox.ini DESTINATION ${CMAKE_BINARY_DIR})

# Install shaders
file(GLOB SHADER_FILES src/renderer/shaders/*.glsl)
file(COPY ${SHADER_FILES} DESTINATION ${CMAKE_BINARY_DIR}/shaders)
