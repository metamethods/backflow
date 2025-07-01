mai2io_lib = static_library(
    'mai2io',
    name_prefix : '',
    include_directories : inc,
    implicit_include_directories : false,
    dependencies : [
        capnhook.get_variable('hook_dep'),
    ],
    sources : [
        'mai2io.c',
        'mai2io.h',
        'config.c',
        'config.h',
    ],
)
