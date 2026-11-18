cmake_minimum_required(VERSION 3.25)
project(clef_vst3 LANGUAGES CXX OBJCXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
set(SDK "${CMAKE_CURRENT_SOURCE_DIR}/../vendor/vst3sdk")
# Compile only the SDK hosting services used here; no sample application/toolchain setup.
add_library(clef_vst3 STATIC
    vst3_bridge.cpp
    plugin_view_mac.mm
    app_lifecycle_mac.mm
    ${SDK}/pluginterfaces/base/funknown.cpp
    ${SDK}/pluginterfaces/base/coreiids.cpp
    ${SDK}/pluginterfaces/base/ustring.cpp
    ${SDK}/public.sdk/source/common/commoniids.cpp
    ${SDK}/public.sdk/source/common/commonstringconvert.cpp
    ${SDK}/public.sdk/source/common/memorystream.cpp
    ${SDK}/public.sdk/source/common/threadchecker_mac.mm
    ${SDK}/public.sdk/source/vst/vstinitiids.cpp
    ${SDK}/public.sdk/source/vst/hosting/connectionproxy.cpp
    ${SDK}/public.sdk/source/vst/hosting/eventlist.cpp
    ${SDK}/public.sdk/source/vst/hosting/hostclasses.cpp
    ${SDK}/public.sdk/source/vst/hosting/module.cpp
    ${SDK}/public.sdk/source/vst/hosting/module_mac.mm
    ${SDK}/public.sdk/source/vst/hosting/parameterchanges.cpp
    ${SDK}/public.sdk/source/vst/hosting/pluginterfacesupport.cpp
    ${SDK}/public.sdk/source/vst/utility/stringconvert.cpp)
set_source_files_properties(${SDK}/public.sdk/source/vst/hosting/module_mac.mm
    PROPERTIES COMPILE_FLAGS "-fobjc-arc")
target_include_directories(clef_vst3 PRIVATE ${SDK})
target_compile_definitions(clef_vst3 PRIVATE RELEASE=1)
target_compile_options(clef_vst3 PRIVATE -Wall -Wextra)
