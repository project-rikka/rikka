package sh.rikka.client.resource.legacy;

import java.util.Arrays;
import java.util.function.IntUnaryOperator;

public final class Raster {
	static final int TRANSPARENT = 0;
	static final int FLIP_X = 1;
	static final int FLIP_Y = 2;
	static final int TRANSPOSE = 4;

	private final int width;
	private final int height;
	private final int[] pixels;

	private Raster(int width, int height, int[] pixels) {
		if (width <= 0 || height <= 0 || pixels.length != width * height) {
			throw new IllegalArgumentException("invalid raster " + width + "x" + height + " with " + pixels.length + " pixels");
		}
		this.width = width;
		this.height = height;
		this.pixels = pixels;
	}

	public static Raster of(int width, int height, int[] argb) {
		return new Raster(width, height, argb);
	}

	public static Raster blank(int width, int height) {
		return new Raster(width, height, new int[width * height]);
	}

	public int width() {
		return this.width;
	}

	public int height() {
		return this.height;
	}

	int get(int x, int y) {
		return this.pixels[y * this.width + x];
	}

	void set(int x, int y, int argb) {
		this.pixels[y * this.width + x] = argb;
	}

	public double scale(int referenceWidth) {
		return (double) this.width / referenceWidth;
	}

	public Raster copy() {
		return new Raster(this.width, this.height, this.pixels.clone());
	}

	public Raster crop(Rect rect) {
		Raster result = blank(Math.max(rect.width(), 1), Math.max(rect.height(), 1));
		result.draw(this, rect, 0, 0);
		return result;
	}

	public Raster resized(int newWidth, int newHeight) {
		Raster result = blank(newWidth, newHeight);
		for (int y = 0; y < newHeight; y++) {
			int sy = (int) ((long) y * this.height / newHeight);
			for (int x = 0; x < newWidth; x++) {
				int sx = (int) ((long) x * this.width / newWidth);
				result.pixels[y * newWidth + x] = this.pixels[sy * this.width + sx];
			}
		}
		return result;
	}

	public Raster averaged(int newWidth, int newHeight) {
		Raster result = blank(newWidth, newHeight);
		for (int y = 0; y < newHeight; y++) {
			int top = (int) ((long) y * this.height / newHeight);
			int bottom = Math.max(top + 1, (int) ((long) (y + 1) * this.height / newHeight));
			for (int x = 0; x < newWidth; x++) {
				int left = (int) ((long) x * this.width / newWidth);
				int right = Math.max(left + 1, (int) ((long) (x + 1) * this.width / newWidth));
				long alpha = 0;
				long red = 0;
				long green = 0;
				long blue = 0;
				int count = 0;
				for (int sy = top; sy < bottom; sy++) {
					for (int sx = left; sx < right; sx++) {
						int pixel = this.pixels[sy * this.width + sx];
						int a = Colors.alpha(pixel);
						alpha += a;
						red += (long) Colors.red(pixel) * a;
						green += (long) Colors.green(pixel) * a;
						blue += (long) Colors.blue(pixel) * a;
						count++;
					}
				}
				result.pixels[y * newWidth + x] = alpha == 0
					? TRANSPARENT
					: Colors.argb((int) (alpha / count), (int) (red / alpha), (int) (green / alpha), (int) (blue / alpha));
			}
		}
		return result;
	}

	Raster canvas(int newWidth, int newHeight) {
		Raster result = blank(newWidth, newHeight);
		result.draw(this, 0, 0);
		return result;
	}

	Raster edges(int side) {
		Raster result = blank(side * 2, this.height);
		result.draw(this, Rect.of(0, 0, side, this.height), 0, 0);
		result.draw(this, Rect.of(this.width - side, 0, side, this.height), side, 0);
		return result;
	}

	public void draw(Raster source, int x, int y) {
		draw(source, new Rect(0, 0, source.width, source.height), x, y);
	}

	public void draw(Raster source, Rect from, int x, int y) {
		for (int row = 0; row < from.height(); row++) {
			int sy = from.y() + row;
			int ty = y + row;
			if (sy < 0 || sy >= source.height || ty < 0 || ty >= this.height) {
				continue;
			}
			for (int column = 0; column < from.width(); column++) {
				int sx = from.x() + column;
				int tx = x + column;
				if (sx < 0 || sx >= source.width || tx < 0 || tx >= this.width) {
					continue;
				}
				this.pixels[ty * this.width + tx] = source.pixels[sy * source.width + sx];
			}
		}
	}

	void fill(Rect rect, int argb) {
		for (int y = Math.max(rect.y(), 0); y < Math.min(rect.bottom(), this.height); y++) {
			int start = y * this.width;
			Arrays.fill(this.pixels, start + Math.max(rect.x(), 0), start + Math.min(rect.right(), this.width), argb);
		}
	}

	public Raster flippedHorizontally() {
		Raster result = blank(this.width, this.height);
		for (int y = 0; y < this.height; y++) {
			for (int x = 0; x < this.width; x++) {
				result.pixels[y * this.width + this.width - 1 - x] = this.pixels[y * this.width + x];
			}
		}
		return result;
	}

	public Raster flippedVertically() {
		Raster result = blank(this.width, this.height);
		for (int y = 0; y < this.height; y++) {
			System.arraycopy(this.pixels, y * this.width, result.pixels, (this.height - 1 - y) * this.width, this.width);
		}
		return result;
	}

	public Raster rotatedClockwise() {
		Raster result = blank(this.height, this.width);
		for (int y = 0; y < this.height; y++) {
			for (int x = 0; x < this.width; x++) {
				result.pixels[x * this.height + this.height - 1 - y] = this.pixels[y * this.width + x];
			}
		}
		return result;
	}

	public Raster unoriented(int orientation) {
		boolean transpose = (orientation & TRANSPOSE) != 0;
		int sourceWidth = transpose ? this.height : this.width;
		int sourceHeight = transpose ? this.width : this.height;
		Raster result = blank(sourceWidth, sourceHeight);
		for (int y = 0; y < this.height; y++) {
			for (int x = 0; x < this.width; x++) {
				int sx = transpose ? y : x;
				int sy = transpose ? x : y;
				if ((orientation & FLIP_X) != 0) {
					sx = sourceWidth - 1 - sx;
				}
				if ((orientation & FLIP_Y) != 0) {
					sy = sourceHeight - 1 - sy;
				}
				result.pixels[sy * sourceWidth + sx] = this.pixels[y * this.width + x];
			}
		}
		return result;
	}

	public Raster rotated(int quarterTurns) {
		Raster result = this;
		for (int i = 0; i < Math.floorMod(quarterTurns, 4); i++) {
			result = result.rotatedClockwise();
		}
		return result == this ? copy() : result;
	}

	public Raster mapped(IntUnaryOperator function) {
		int[] mapped = new int[this.pixels.length];
		for (int i = 0; i < mapped.length; i++) {
			mapped[i] = function.applyAsInt(this.pixels[i]);
		}
		return new Raster(this.width, this.height, mapped);
	}

	public boolean isTransparent(Rect rect) {
		for (int y = Math.max(rect.y(), 0); y < Math.min(rect.bottom(), this.height); y++) {
			for (int x = Math.max(rect.x(), 0); x < Math.min(rect.right(), this.width); x++) {
				if (Colors.alpha(this.pixels[y * this.width + x]) != 0) {
					return false;
				}
			}
		}
		return true;
	}

	public boolean isTransparent() {
		return isTransparent(new Rect(0, 0, this.width, this.height));
	}
}
