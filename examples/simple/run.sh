export DYLD_LIBRARY_PATH=$(rustc --print sysroot)/lib
export DYLD_INSERT_LIBRARIES=/Users/id/devel/Rust/fishhook/examples/memhook/target/debug/libmemhook.dylib
#export DYLD_FORCE_FLAT_NAMESPACE=1
/Users/id/devel/Rust/fishhook/examples/simple/target/debug/simple
