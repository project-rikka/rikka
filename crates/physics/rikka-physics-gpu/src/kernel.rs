use rikka_gpu::Kernel;
use rikka_gpu::{Compute, Result};
use rikka_spirv::Shader;

#[derive(Debug)]
pub struct NamedKernel {
    kernel: Kernel,
    name: &'static str,
    push_bytes: usize,
}

impl NamedKernel {
    pub fn load(compute: &Compute, shader: Shader, name: &'static str) -> Result<NamedKernel> {
        Ok(NamedKernel {
            kernel: compute.kernel(shader)?,
            name,
            push_bytes: shader.push_bytes as usize,
        })
    }

    #[must_use]
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn push_bytes(&self) -> usize {
        self.push_bytes
    }
}

#[macro_export]
macro_rules! kernels {
    ($vis:vis struct $kernels:ident from $shaders:ident { $($name:ident => $spirv:ident),* $(,)? }) => {
        $vis struct $kernels {
            $($vis $name: $crate::NamedKernel,)*
        }

        impl $kernels {
            $vis fn new(
                compute: &$crate::private::Compute,
            ) -> $crate::private::Result<$kernels> {
                Ok($kernels {
                    $($name: $crate::NamedKernel::load(
                        compute,
                        $shaders::$spirv,
                        stringify!($name),
                    )?,)*
                })
            }
        }
    };
}
