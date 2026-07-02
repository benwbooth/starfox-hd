cmake_minimum_required(VERSION 3.16)
project(starfox-hd C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find dependencies
find_package(PkgConfig REQUIRED)
pkg_check_modules(SDL2 REQUIRED sdl2)

find_package(OpenGL REQUIRED)

# Glad (vendored OpenGL loader)
add_library(glad STATIC src/glad/src/glad.c)
target_include_directories(glad PUBLIC src/glad/include)

# snes_spc (vendored SPC700 emulator, C++)
add_library(snes_spc STATIC
    src/snes_spc/spc.cpp
    src/snes_spc/dsp.cpp
    src/snes_spc/SNES_SPC.cpp
    src/snes_spc/SNES_SPC_misc.cpp
    src/snes_spc/SNES_SPC_state.cpp
    src/snes_spc/SPC_DSP.cpp
    src/snes_spc/SPC_Filter.cpp
)
target_include_directories(snes_spc PUBLIC src/snes_spc)
target_compile_options(snes_spc PRIVATE -w)  # Suppress warnings in vendored code
target_compile_definitions(snes_spc PRIVATE BLARGG_BUILD_DLL)  # Enable debug_printf no-op

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
    src/renderer/font.c
    src/renderer/sprites.c
    src/renderer/bg2d.c
    src/renderer/ui.c
    src/audio/audio.c
    src/audio/spc_player.c
    src/audio/spc_boot.c
    src/game/boot.c
    src/game/nmi.c
    src/game/obj.c
    src/game/game.c
    src/game/coldet.c
    src/game/draw.c
    src/game/alien_compat.c
    src/game/game_vars.c
    src/game/sound.c
    src/game/bgs.c
    src/game/windows.c
    src/game/strings.c
    src/game/messages_data.c
    src/game/world.c
    src/game/planets.c
    src/strat/strat_common.c
    src/strat/strat_player.c
    src/strat/strat_ground.c
    src/strat/strat_enemy.c
    src/strat/strat_boss2.c
    src/strat/strat_boss_sea.c
    src/strat/strat_boss8.c
    src/strat/istrat_shapes.c
    src/strat/strat_table.c
    src/path/paths.c
    src/path/path_catalog.c
    src/path/path_literals.c
    src/map/map_exec.c
    src/map/levels.c
    src/map/map_catalog.c
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
    snes_spc
    m
)

target_compile_options(starfox-hd PRIVATE
    ${SDL2_CFLAGS_OTHER}
    -Wall -Wextra -Wno-unused-parameter
)

# Standalone mesh viewer executable
add_executable(starfox-mesh-viewer
    src/viewer/mesh_viewer_main.c
    src/viewer/shape_catalog_asm.c
    src/renderer/gl_backend.c
    src/renderer/transform.c
)

target_include_directories(starfox-mesh-viewer PRIVATE
    src
    ${SDL2_INCLUDE_DIRS}
    ${OPENGL_INCLUDE_DIR}
)

target_link_libraries(starfox-mesh-viewer PRIVATE
    ${SDL2_LIBRARIES}
    OpenGL::GL
    glad
    m
)

target_compile_options(starfox-mesh-viewer PRIVATE
    ${SDL2_CFLAGS_OTHER}
    -Wall -Wextra -Wno-unused-parameter
)

# Copy assets
file(COPY ${CMAKE_SOURCE_DIR}/starfox.ini DESTINATION ${CMAKE_BINARY_DIR})
file(COPY ${CMAKE_SOURCE_DIR}/data/snd DESTINATION ${CMAKE_BINARY_DIR}/data)
file(COPY ${CMAKE_SOURCE_DIR}/data/font DESTINATION ${CMAKE_BINARY_DIR}/data)
file(COPY ${CMAKE_SOURCE_DIR}/data/sprites DESTINATION ${CMAKE_BINARY_DIR}/data)
file(COPY ${CMAKE_SOURCE_DIR}/data/title DESTINATION ${CMAKE_BINARY_DIR}/data)
file(COPY ${CMAKE_SOURCE_DIR}/data/bg DESTINATION ${CMAKE_BINARY_DIR}/data)
file(COPY ${CMAKE_SOURCE_DIR}/data/map DESTINATION ${CMAKE_BINARY_DIR}/data)

# Install shaders
file(GLOB SHADER_FILES src/renderer/shaders/*.glsl)
file(COPY ${SHADER_FILES} DESTINATION ${CMAKE_BINARY_DIR}/shaders)
