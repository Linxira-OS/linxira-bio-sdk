# Native Kernel Boundary

This directory is reserved for C++ or GPU kernels that a reproducible
benchmark has justified; Rust remains the default compute runtime.

Do not add a second implementation of the Rust I/O, capability, or CLI layers.
Introduce a kernel only after a reproducible benchmark identifies the exact hot
path, expected speed or memory target, scientific-equivalence fixture, and ABI
boundary.

The future canonical build uses standalone CMake with Ninja and LLVM/Clang or
MinGW on Windows, and GCC or Clang on Linux. Visual Studio solution files are
not the project build interface.
