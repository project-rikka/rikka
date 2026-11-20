# Sample Blender files

`scene.blend`, `library.blend` and `linking.blend` were made for these tests with Blender 5.2
by a script and carry no third-party content. `linking.blend` links the `shared` collection
from `library.blend` through a relative path.

`studio.blend` (zstd) was made for these tests with Blender 5.2 by a script: collections,
every modifier kind the scene describes, an armature with bone collections and constraints, shape
keys, vertex groups, UV maps, colour attributes, custom normals and a PBR material with packed
2×2 PNG textures. It carries no third-party content and is released under CC0 1.0.

`template.blend.zst` is compiled into the library. It was saved by Blender 5.2.2 from a scene
holding one ID of every kind the writer emits (its SDNA, default struct values and the Layout
workspace user interface) and is released under CC0 1.0. Its "Smooth by Angle" node group comes
from Blender's bundled essentials asset library, which Blender distributes under CC0 1.0.

`TorusLightsCams_250_compressed.blend` (Blender 2.50, gzip) comes unchanged from the
`test/models/BLEND/` folder of [assimp](https://github.com/assimp/assimp) and is used under its
BSD 3-Clause license:

Copyright (c) 2006-2026, assimp team
All rights reserved.

Redistribution and use of this software in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this list of
  conditions and the following disclaimer.

* Redistributions in binary form must reproduce the above copyright notice, this list of
  conditions and the following disclaimer in the documentation and/or other materials provided
  with the distribution.

* Neither the name of the assimp team, nor the names of its contributors may be used to endorse
  or promote products derived from this software without specific prior written permission of
  the assimp team.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
