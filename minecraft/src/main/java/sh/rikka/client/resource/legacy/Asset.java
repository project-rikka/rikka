package sh.rikka.client.resource.legacy;

import java.io.IOException;

public final class Asset {
	private final ByteSource source;
	private final Object lock = new Object();
	private RasterSource image;
	private Raster raster;
	private byte[] bytes;

	private Asset(ByteSource source, RasterSource image) {
		this.source = source;
		this.image = image;
	}

	static Asset stored(PackArchive archive, String entry) {
		return new Asset(() -> archive.read(entry), null);
	}

	static Asset image(RasterSource image) {
		return new Asset(null, image);
	}

	static Asset bytes(byte[] bytes) {
		return new Asset(() -> bytes, null);
	}

	static Asset encoded(ByteSource source) {
		return new Asset(source, null);
	}

	public byte[] bytes() throws IOException {
		if (this.source != null) {
			return this.source.read();
		}
		synchronized (this.lock) {
			if (this.bytes == null) {
				this.bytes = PngEncoder.encode(generated());
				this.image = null;
				this.raster = null;
			}
			return this.bytes;
		}
	}

	Raster raster(ImageDecoder decoder) throws IOException {
		if (this.source != null) {
			return decoder.decode(this.source.read());
		}
		synchronized (this.lock) {
			return this.bytes != null ? decoder.decode(this.bytes) : generated();
		}
	}

	private Raster generated() throws IOException {
		if (this.raster == null) {
			this.raster = this.image.get();
		}
		return this.raster;
	}

	@FunctionalInterface
	interface ByteSource {
		byte[] read() throws IOException;
	}
}
