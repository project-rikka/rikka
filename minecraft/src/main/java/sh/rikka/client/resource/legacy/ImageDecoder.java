package sh.rikka.client.resource.legacy;

import java.io.IOException;

@FunctionalInterface
public interface ImageDecoder {
	Raster decode(byte[] png) throws IOException;
}
