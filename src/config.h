#ifndef STARFOX_CONFIG_H
#define STARFOX_CONFIG_H

#include "types.h"

typedef struct {
    // [General]
    char rom_file[512];
    char asset_dir[512];

    // [Video]
    int window_width;
    int window_height;
    int fullscreen;
    int target_fps;
    int widescreen;
    int msaa;
    int crt_filter;
    int bloom_enabled;

    // [Audio]
    int sample_rate;
    int buffer_size;
    int volume;

    // [Input]
    float deadzone;
} Config;

// Load config from INI file, returning defaults if file missing
void Config_Load(Config *cfg, const char *path);

// Save current config to INI file
void Config_Save(const Config *cfg, const char *path);

// Set defaults
void Config_SetDefaults(Config *cfg);

#endif // STARFOX_CONFIG_H
