package sh.rikka.client.resource.legacy;

record Rect(int x, int y, int width, int height) {
	public Rect {
		if (width < 0 || height < 0) {
			throw new IllegalArgumentException("negative rect size " + width + "x" + height);
		}
	}

	public static Rect of(int x, int y, int width, int height) {
		return new Rect(x, y, width, height);
	}

	public Rect scaled(double factor) {
		int left = (int) Math.round(this.x * factor);
		int top = (int) Math.round(this.y * factor);
		int right = (int) Math.round((this.x + this.width) * factor);
		int bottom = (int) Math.round((this.y + this.height) * factor);
		return new Rect(left, top, right - left, bottom - top);
	}

	public int right() {
		return this.x + this.width;
	}

	public int bottom() {
		return this.y + this.height;
	}
}
