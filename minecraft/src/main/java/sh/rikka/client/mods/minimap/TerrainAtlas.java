package sh.rikka.client.mods.minimap;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.device.GpuDevice;
import com.mojang.renderpearl.api.textures.FilterMode;
import com.mojang.renderpearl.api.textures.GpuSampler;
import com.mojang.renderpearl.api.textures.GpuTexture;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import net.minecraft.util.ARGB;
import net.minecraft.world.level.ChunkPos;
import org.joml.Vector4f;

final class TerrainAtlas implements AutoCloseable {
	static final int SLOTS = 64;
	static final int SIZE = SLOTS * TerrainScanner.CHUNK;

	private static final long VACANT = Long.MIN_VALUE;
	private static final int CHUNK_PIXELS = TerrainScanner.CHUNK * TerrainScanner.CHUNK;
	private static final int[] TRANSPARENT = new int[CHUNK_PIXELS];

	private final GpuTexture texture;
	private final GpuTextureView view;
	private final long[] occupants = new long[SLOTS * SLOTS];
	private final long[] keys = new long[SLOTS * SLOTS];
	private final ByteBuffer staging = ByteBuffer.allocateDirect(CHUNK_PIXELS * 4).order(ByteOrder.nativeOrder());

	TerrainAtlas() {
		GpuDevice device = RenderSystem.getDevice();
		this.texture = device.createTexture(
			"Rikka minimap terrain",
			GpuTexture.USAGE_COPY_DST | GpuTexture.USAGE_TEXTURE_BINDING | GpuTexture.USAGE_RENDER_ATTACHMENT,
			GpuFormat.RGBA8_UNORM,
			SIZE,
			SIZE,
			1,
			1
		);
		this.view = device.createTextureView(this.texture);
		clear();
	}

	GpuTextureView view() {
		return this.view;
	}

	GpuSampler sampler() {
		return RenderSystem.getSamplerCache().getRepeat(FilterMode.NEAREST);
	}

	void clear() {
		RenderSystem.getDevice().createCommandEncoder().clearColorTexture(this.texture, new Vector4f());
		Arrays.fill(this.occupants, VACANT);
		Arrays.fill(this.keys, VACANT);
	}

	boolean holds(int chunkX, int chunkZ, long key) {
		int slot = slot(chunkX, chunkZ);
		return this.occupants[slot] == ChunkPos.pack(chunkX, chunkZ) && this.keys[slot] == key;
	}

	boolean occupiedBy(int chunkX, int chunkZ) {
		return this.occupants[slot(chunkX, chunkZ)] == ChunkPos.pack(chunkX, chunkZ);
	}

	void write(int chunkX, int chunkZ, long key, int[] argb) {
		this.staging.clear();
		for (int pixel : argb) {
			this.staging.put((byte) ARGB.red(pixel)).put((byte) ARGB.green(pixel)).put((byte) ARGB.blue(pixel)).put((byte) ARGB.alpha(pixel));
		}
		this.staging.flip();
		RenderSystem.getDevice()
			.createCommandEncoder()
			.writeToTexture(
				this.texture,
				this.staging,
				0,
				0,
				Math.floorMod(chunkX, SLOTS) * TerrainScanner.CHUNK,
				Math.floorMod(chunkZ, SLOTS) * TerrainScanner.CHUNK,
				TerrainScanner.CHUNK,
				TerrainScanner.CHUNK
			);
		int slot = slot(chunkX, chunkZ);
		this.occupants[slot] = ChunkPos.pack(chunkX, chunkZ);
		this.keys[slot] = key;
	}

	void vacate(int chunkX, int chunkZ, long key) {
		write(chunkX, chunkZ, key, TRANSPARENT);
	}

	@Override
	public void close() {
		this.view.close();
		this.texture.close();
	}

	private static int slot(int chunkX, int chunkZ) {
		return Math.floorMod(chunkZ, SLOTS) * SLOTS + Math.floorMod(chunkX, SLOTS);
	}
}
