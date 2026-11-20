# Third-party notices

## rikka-psd-render: brightness/contrast curve

`rikka-psd-render/src/adjust.rs` models Photoshop's modern Brightness/Contrast adjustment with the empirical constants published by [psd-tools](https://github.com/psd-tools/psd-tools) in `src/psd_tools/composite/adjustments.py`: the contrast spline control points (63 and 191, moved by 25 per unit of contrast) and the brightness polynomial coefficients (1.65, -1.0, 1.96, 1.0, 1.0) with exponents (0.35, 10.0, 0.4, 4.0, 1.25).

```
Copyright (c) 2019 Kota Yamaguchi

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
