#pragma once

/*
   CHU2TO3 CUSTOM IO API

   This dll just mirrors chuniio dll binds but with a dynamic library loading and 
   a SHMEM system to let a single 32bit dll talk with x86 and x64 processes at once
*/

#include <windows.h>

#include <stdbool.h>
#include <stdint.h>

uint16_t chu2to3_io_get_api_version(void);
HRESULT chu2to3_io_jvs_init(void);
void chu2to3_io_jvs_poll(uint8_t *opbtn, uint8_t *beams);
void chu2to3_io_jvs_read_coin_counter(uint16_t *total);
HRESULT chu2to3_io_slider_init(void);
typedef void (*chuni_io_slider_callback_t)(const uint8_t *state);
void chu2to3_io_slider_start(chuni_io_slider_callback_t callback);
void chu2to3_io_slider_stop(void);
void chu2to3_io_slider_set_leds(const uint8_t *rgb);
HRESULT chu2to3_io_led_init(void);
void chu2to3_io_led_set_colors(uint8_t board, uint8_t *rgb);
uint16_t chu2to3_load_dll(const wchar_t *dllname);
