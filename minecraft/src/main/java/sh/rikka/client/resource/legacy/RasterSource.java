package sh.rikka.client.resource.legacy;

import java.io.IOException;

@FunctionalInterface
interface RasterSource {
	Raster get() throws IOException;
}
