package sh.rikka.client.resource.legacy;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.util.zip.CRC32;
import java.util.zip.Deflater;
import java.util.zip.DeflaterOutputStream;

final class PngEncoder {
	private static final byte[] SIGNATURE = {(byte) 0x89, 'P', 'N', 'G', '\r', '\n', 0x1A, '\n'};
	private static final int BIT_DEPTH = 8;
	private static final int COLOR_TYPE_RGBA = 6;
	private static final int FILTER_SUB = 1;
	private static final int BYTES_PER_PIXEL = 4;

	private PngEncoder() {
	}

	public static byte[] encode(Raster raster) {
		try {
			ByteArrayOutputStream file = new ByteArrayOutputStream();
			file.write(SIGNATURE);
			ByteArrayOutputStream header = new ByteArrayOutputStream();
			DataOutputStream headerData = new DataOutputStream(header);
			headerData.writeInt(raster.width());
			headerData.writeInt(raster.height());
			headerData.writeByte(BIT_DEPTH);
			headerData.writeByte(COLOR_TYPE_RGBA);
			headerData.writeByte(0);
			headerData.writeByte(0);
			headerData.writeByte(0);
			chunk(file, "IHDR", header.toByteArray());
			chunk(file, "IDAT", compress(raster));
			chunk(file, "IEND", new byte[0]);
			return file.toByteArray();
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to encode PNG", failure);
		}
	}

	private static byte[] compress(Raster raster) throws IOException {
		int width = raster.width();
		int stride = width * BYTES_PER_PIXEL;
		byte[] row = new byte[stride];
		byte[] filtered = new byte[stride + 1];
		filtered[0] = FILTER_SUB;
		ByteArrayOutputStream compressed = new ByteArrayOutputStream();
		Deflater deflater = new Deflater(Deflater.BEST_SPEED);
		try (DeflaterOutputStream out = new DeflaterOutputStream(compressed, deflater)) {
			for (int y = 0; y < raster.height(); y++) {
				for (int x = 0; x < width; x++) {
					int argb = raster.get(x, y);
					int offset = x * BYTES_PER_PIXEL;
					row[offset] = (byte) Colors.red(argb);
					row[offset + 1] = (byte) Colors.green(argb);
					row[offset + 2] = (byte) Colors.blue(argb);
					row[offset + 3] = (byte) Colors.alpha(argb);
				}
				for (int i = 0; i < stride; i++) {
					int left = i >= BYTES_PER_PIXEL ? row[i - BYTES_PER_PIXEL] : 0;
					filtered[i + 1] = (byte) (row[i] - left);
				}
				out.write(filtered);
			}
		} finally {
			deflater.end();
		}
		return compressed.toByteArray();
	}

	private static void chunk(ByteArrayOutputStream file, String type, byte[] data) throws IOException {
		DataOutputStream out = new DataOutputStream(file);
		byte[] typeBytes = type.getBytes(StandardCharsets.US_ASCII);
		out.writeInt(data.length);
		out.write(typeBytes);
		out.write(data);
		CRC32 crc = new CRC32();
		crc.update(typeBytes);
		crc.update(data);
		out.writeInt((int) crc.getValue());
	}
}
