# Native Kernel Boundary

This directory is reserved for benchmark-justified C++ or GPU kernels.

Do not add a second implementation of the Rust I/O, capability, or CLI layers.
Introduce a kernel only after a reproducible benchmark identifies the exact hot
path, expected speed or memory target, scientific-equivalence fixture, and ABI
boundary.

The future canonical build uses standalone CMake with Ninja and LLVM/Clang or
MinGW on Windows, and GCC or Clang on Linux. Visual Studio solution files are
not the project build interface.
