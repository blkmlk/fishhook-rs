export DYLD_LIBRARY_PATH=$(rustc --print sysroot)/lib
export DYLD_INSERT_LIBRARIES=../memhook/target/release/libmemhook.dylib
export DYLD_FORCE_FLAT_NAMESPACE=1
./target/debug/simple