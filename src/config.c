#include "config.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

void Config_SetDefaults(Config *cfg) {
    memset(cfg, 0, sizeof(*cfg));
    strncpy(cfg->rom_file, "Star Fox (USA) (Rev 2).sfc", sizeof(cfg->rom_file) - 1);
    strncpy(cfg->asset_dir, "data", sizeof(cfg->asset_dir) - 1);
    cfg->window_width = 1280;
    cfg->window_height = 720;
    cfg->fullscreen = 0;
    cfg->target_fps = 0;
    cfg->widescreen = 0;
    cfg->msaa = 4;
    cfg->crt_filter = 0;
    cfg->bloom_enabled = 0;
    cfg->sample_rate = 48000;
    cfg->buffer_size = 1024;
    cfg->volume = 100;
    cfg->deadzone = 0.15f;
}

static char *trim(char *s) {
    while (isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s) - 1;
    while (end > s && isspace((unsigned char)*end)) *end-- = '\0';
    return s;
}

void Config_Load(Config *cfg, const char *path) {
    Config_SetDefaults(cfg);

    FILE *f = fopen(path, "r");
    if (!f) {
        printf("Config: %s not found, using defaults\n", path);
        return;
    }

    char line[1024];
    char section[64] = "";

    while (fgets(line, sizeof(line), f)) {
        char *s = trim(line);
        if (*s == '\0' || *s == '#' || *s == ';') continue;

        if (*s == '[') {
            char *end = strchr(s, ']');
            if (end) {
                *end = '\0';
                strncpy(section, s + 1, sizeof(section) - 1);
            }
            continue;
        }

        char *eq = strchr(s, '=');
        if (!eq) continue;

        *eq = '\0';
        char *key = trim(s);
        char *val = trim(eq + 1);

        if (strcmp(section, "General") == 0) {
            if (strcmp(key, "RomFile") == 0)
                strncpy(cfg->rom_file, val, sizeof(cfg->rom_file) - 1);
            else if (strcmp(key, "AssetDir") == 0)
                strncpy(cfg->asset_dir, val, sizeof(cfg->asset_dir) - 1);
        } else if (strcmp(section, "Video") == 0) {
            if (strcmp(key, "WindowWidth") == 0)       cfg->window_width = atoi(val);
            else if (strcmp(key, "WindowHeight") == 0)  cfg->window_height = atoi(val);
            else if (strcmp(key, "Fullscreen") == 0)    cfg->fullscreen = atoi(val);
            else if (strcmp(key, "TargetFPS") == 0)     cfg->target_fps = atoi(val);
            else if (strcmp(key, "Widescreen") == 0)    cfg->widescreen = atoi(val);
            else if (strcmp(key, "MSAA") == 0)          cfg->msaa = atoi(val);
            else if (strcmp(key, "CRTFilter") == 0)     cfg->crt_filter = atoi(val);
            else if (strcmp(key, "BloomEnabled") == 0)   cfg->bloom_enabled = atoi(val);
        } else if (strcmp(section, "Audio") == 0) {
            if (strcmp(key, "SampleRate") == 0)         cfg->sample_rate = atoi(val);
            else if (strcmp(key, "BufferSize") == 0)    cfg->buffer_size = atoi(val);
            else if (strcmp(key, "Volume") == 0)        cfg->volume = atoi(val);
        } else if (strcmp(section, "Input") == 0) {
            if (strcmp(key, "DeadZone") == 0)           cfg->deadzone = (float)atof(val);
        }
    }

    fclose(f);
    printf("Config: loaded %s\n", path);
}

void Config_Save(const Config *cfg, const char *path) {
    FILE *f = fopen(path, "w");
    if (!f) {
        printf("Config: failed to save %s\n", path);
        return;
    }

    fprintf(f, "[General]\n");
    fprintf(f, "RomFile = %s\n", cfg->rom_file);
    fprintf(f, "AssetDir = %s\n", cfg->asset_dir);
    fprintf(f, "\n[Video]\n");
    fprintf(f, "WindowWidth = %d\n", cfg->window_width);
    fprintf(f, "WindowHeight = %d\n", cfg->window_height);
    fprintf(f, "Fullscreen = %d\n", cfg->fullscreen);
    fprintf(f, "TargetFPS = %d\n", cfg->target_fps);
    fprintf(f, "Widescreen = %d\n", cfg->widescreen);
    fprintf(f, "MSAA = %d\n", cfg->msaa);
    fprintf(f, "CRTFilter = %d\n", cfg->crt_filter);
    fprintf(f, "BloomEnabled = %d\n", cfg->bloom_enabled);
    fprintf(f, "\n[Audio]\n");
    fprintf(f, "SampleRate = %d\n", cfg->sample_rate);
    fprintf(f, "BufferSize = %d\n", cfg->buffer_size);
    fprintf(f, "Volume = %d\n", cfg->volume);
    fprintf(f, "\n[Input]\n");
    fprintf(f, "DeadZone = %.2f\n", cfg->deadzone);

    fclose(f);
}
