# kanirenderer
🦀 renderer, a basic renderer for previewing your 3D meshes/file quickly with a command line.

![PREVIEW](https://github.com/ourbunka/kanirenderer/blob/main/preview.gif)
![PREVIEW2](https://github.com/ourbunka/kanirenderer/blob/main/preview2.gif)

in /res directory, open terminal
  > kanirenderer sponza.obj opengl fullscreen

# features
-currently support .obj file with png/jpeg textures,

-switch between render modes with "Tab" key
  -unlit
  -lit
  -lit with shadow
  -wireframe

-basic lighting (diffuse, specular and ambient light),

-normal map,

-basic shadow map,

-mesh with OPENGL(meshes authored in Blender) or DIRECTX format,

-fullscreen or windowed mode,

-basic FPS movement(wasd + mouse),

-basic movable point light and light controller


# How to use?

  1)compile with cargo build --release
  
  2)add kanirenderer.exe dir to your OS ENVIRONMENT "PATH" VARIABLE
  
  3)in your obj file directory, open terminal, then enter "kanirenderer yourfilename.obj default windowed"


# Credit
sponza.obj sample file included in /res is originally created by Frank Meinl
