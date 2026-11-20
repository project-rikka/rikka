+++
name = "Measure"

[params]
document = "Tab name; the active tab when omitted."
area = "`selection`, `whole` or [x, y, width, height]; the selection if any, else the whole. With only `points`, just the points."
layer = "A layer index or name to measure alone instead of the composite."
points = "[[x, y], …], at most 64, read as #rrggbb."
radius = "Average a (2·radius+1)² square per point, 0–50; 0 when omitted."
+++

Reads numbers in sRGB 0–255: pixels, mean R G B and luma, luma percentiles 1/5/50/95/99, clipped shadows and highlights, mean saturation, midtone cast (warm/cool, green/magenta) and a 16-bin luma histogram in %.
