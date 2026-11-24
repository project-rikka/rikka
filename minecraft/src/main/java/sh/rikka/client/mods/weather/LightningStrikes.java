package sh.rikka.client.mods.weather;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.LightningBolt;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.phys.Vec3;

final class LightningStrikes {
	private static final double TICKS_PER_MINUTE = 1200.0;
	private static final double MIN_DISTANCE = 16.0;
	private static final double MAX_DISTANCE = 128.0;
	private static final double BLOCKS_PER_CHUNK = 16.0;
	private static final int FIRST_ENTITY_ID = -2;

	private int nextEntityId = FIRST_ENTITY_ID;

	void tick(ClientLevel level, int perMinute, float thunder) {
		if (perMinute <= 0) {
			return;
		}
		RandomSource random = level.getRandom();
		if (random.nextDouble() >= perMinute / TICKS_PER_MINUTE * thunder) {
			return;
		}
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		if (player == null) {
			return;
		}
		double reach = Math.clamp(minecraft.options.renderDistance().get() * BLOCKS_PER_CHUNK, MIN_DISTANCE, MAX_DISTANCE);
		double distance = MIN_DISTANCE + random.nextDouble() * (reach - MIN_DISTANCE);
		double angle = random.nextDouble() * Math.TAU;
		int x = (int) Math.floor(player.getX() + Math.cos(angle) * distance);
		int z = (int) Math.floor(player.getZ() + Math.sin(angle) * distance);
		if (!level.getChunkSource().hasChunk(x >> 4, z >> 4)) {
			return;
		}
		strike(level, x, level.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z), z);
	}

	private void strike(ClientLevel level, int x, int y, int z) {
		LightningBolt bolt = EntityTypes.LIGHTNING_BOLT.create(level, EntitySpawnReason.EVENT);
		if (bolt == null) {
			return;
		}
		bolt.setId(this.nextEntityId--);
		bolt.snapTo(new Vec3(x + 0.5, y, z + 0.5));
		level.addEntity(bolt);
	}
}
