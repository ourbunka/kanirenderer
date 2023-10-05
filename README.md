# kanirenderer
🦀 renderer, a basic renderer for preview your 3D meshes/file quickly with a command line.

in /res directory, open terminal
  > kanirenderer sponza.obj opengl fullscreen

1st variable is your obj file name,

2nd variable can be opengl or default(directx)

3rd variable is optional, can be fullscreen or windowed(default)

# features
-currently support .obj file with png/jpeg textures,

-basic lighting with albedo, specular and normal map support.

-support mesh with OPENGL(ie meshes authored in Blender) or DIRECTX format,

-fullscreen or windowed mode,

-basic FPS movement(wasd + mouse),

-using WGPU graphic api than compiled to native graphic api like DIRECTX12 / VULKAN / METAL

-windows 10/11 executables included, (Linux and MacOS should be supported,but you will need to compiled it yourself. install rust compiler , git clone this repo and in project dir "cargo build --release",

-DirectX12 compatibles GPU required for Windows 10/11, Vulkan for Linux and Metal for macos.

# How to use?

  1)compile yourself or download the executable
  
  2)add kanirenderer.exe executable dir PATH to your OS ENVIRONMENT "PATH" VARIABLE
  
  3)in your obj file directory, open terminal, then enter "kanirenderer yourfilename.obj opengl windowed"


# Credit
sponza.obj sample file included in /res is originally created by Frank Meinl
