package sh.rikka.client.resource;

import com.mojang.blaze3d.platform.NativeImage;
import java.io.IOException;
import sh.rikka.client.resource.legacy.ImageDecoder;
import sh.rikka.client.resource.legacy.Raster;

final class NativeImageDecoder implements ImageDecoder {
	static final NativeImageDecoder INSTANCE = new NativeImageDecoder();

	private NativeImageDecoder() {
	}

	@Override
	public Raster decode(byte[] png) throws IOException {
		try (NativeImage image = NativeImage.read(png)) {
			return Raster.of(image.getWidth(), image.getHeight(), image.getPixels());
		}
	}
}
