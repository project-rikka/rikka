package sh.rikka.bench;

import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.state.BlockState;

final class Vault {
	private static final int ORIGIN_X = -400;
	private static final int ORIGIN_Z = 400;
	private static final int CLEAR_MARGIN = 10;
	private static final int CLEAR_HEIGHT = 14;
	private static final int FLOOR_SIZE = 40;
	private static final int BOX_MIN_X = 12;
	private static final int BOX_MAX_X = 36;
	private static final int BOX_MIN_Z = 4;
	private static final int BOX_MAX_Z = 28;
	private static final int STOREY = 5;
	private static final int STOREYS = 2;
	private static final int WINDOW_Y = 2;
	private static final int WINDOW_Z = 16;
	private static final int MOB_FIRST_X = 15;
	private static final int MOB_LAST_X = 33;
	private static final int MOB_SPACING_X = 2;
	private static final double MOB_FIRST_Z = 6.0;
	private static final double MOB_LAST_Z = 26.0;
	private static final double MOB_SPACING_Z = 1.5;
	private static final int OPEN_MIN_Z = 31;
	private static final int OPEN_MAX_Z = 37;
	private static final int OPEN_SPACING = 2;
	private static final int OPEN_CHEST_Z = 39;
	private static final int BANNER_SPACING = 4;
	private static final int FLAGS = Block.UPDATE_CLIENTS;
	private static final float FACING_CAMERA = 90.0F;
	private static final List<EntityType<? extends Entity>> MOBS = List.of(
		EntityTypes.COW,
		EntityTypes.SHEEP,
		EntityTypes.PIG,
		EntityTypes.CHICKEN,
		EntityTypes.VILLAGER,
		EntityTypes.HORSE,
		EntityTypes.IRON_GOLEM,
		EntityTypes.WOLF
	);

	private Vault() {
	}

	static Pose view(int ground) {
		return new Pose(ORIGIN_X + 2.5, floor(ground) + 1.0, ORIGIN_Z + 22.5, -90.0F, 4.0F);
	}

	static void build(MinecraftServer server, int ground) {
		ServerLevel level = server.overworld();
		int floor = floor(ground);
		BlockState air = Blocks.AIR.defaultBlockState();
		BlockState stone = Blocks.STONE.defaultBlockState();
		for (int x = -CLEAR_MARGIN; x < FLOOR_SIZE + CLEAR_MARGIN; x++) {
			for (int z = -CLEAR_MARGIN; z < FLOOR_SIZE + CLEAR_MARGIN; z++) {
				for (int y = 0; y <= CLEAR_HEIGHT; y++) {
					boolean paved = y == 0 && x >= 0 && x < FLOOR_SIZE && z >= 0 && z < FLOOR_SIZE;
					set(level, x, floor + y, z, paved ? stone : air);
				}
			}
		}
		buildBox(level, floor, stone);
		for (int storey = 0; storey < STOREYS; storey++) {
			int base = floor + storey * STOREY;
			furnishStorey(level, base);
			int index = storey;
			for (int x = MOB_FIRST_X; x <= MOB_LAST_X; x += MOB_SPACING_X) {
				for (double z = MOB_FIRST_Z; z <= MOB_LAST_Z; z += MOB_SPACING_Z) {
					spawn(level, MOBS.get(index++ % MOBS.size()), x + 0.5, base + 1, z + 0.5);
				}
			}
		}
		int index = 0;
		for (int x = MOB_FIRST_X; x <= MOB_LAST_X; x += OPEN_SPACING) {
			for (int z = OPEN_MIN_Z; z <= OPEN_MAX_Z; z += OPEN_SPACING) {
				spawn(level, MOBS.get(index++ % MOBS.size()), x + 0.5, floor + 1, z + 0.5);
			}
			set(level, x, floor + 1, OPEN_CHEST_Z, chest());
		}
	}

	private static void buildBox(ServerLevel level, int floor, BlockState stone) {
		int top = floor + STOREY * STOREYS;
		for (int x = BOX_MIN_X; x <= BOX_MAX_X; x++) {
			for (int z = BOX_MIN_Z; z <= BOX_MAX_Z; z++) {
				boolean wall = x == BOX_MIN_X || x == BOX_MAX_X || z == BOX_MIN_Z || z == BOX_MAX_Z;
				for (int y = floor + 1; y <= top; y++) {
					if (wall || (y - floor) % STOREY == 0) {
						set(level, x, y, z, stone);
					}
				}
			}
		}
		set(level, BOX_MIN_X, floor + WINDOW_Y, WINDOW_Z, Blocks.AIR.defaultBlockState());
	}

	private static void furnishStorey(ServerLevel level, int base) {
		for (int z = BOX_MIN_Z + 1; z < BOX_MAX_Z; z++) {
			set(level, BOX_MAX_X - 1, base + 1, z, chest());
			if (z % BANNER_SPACING == 0 && z != WINDOW_Z) {
				set(level, BOX_MIN_X + 1, base + 1, z, Blocks.BANNER.red().defaultBlockState());
			}
		}
	}

	private static BlockState chest() {
		return Blocks.CHEST.defaultBlockState().setValue(ChestBlock.FACING, Direction.WEST);
	}

	private static void spawn(ServerLevel level, EntityType<? extends Entity> type, double x, double y, double z) {
		Entity entity = type.create(level, EntitySpawnReason.COMMAND);
		if (entity == null) {
			throw new IllegalStateException("failed to create a vault entity");
		}
		entity.snapTo(ORIGIN_X + x, y, ORIGIN_Z + z, FACING_CAMERA, 0.0F);
		entity.setNoGravity(true);
		entity.setSilent(true);
		if (entity instanceof Mob mob) {
			mob.setNoAi(true);
			mob.setPersistenceRequired();
			mob.setYHeadRot(FACING_CAMERA);
			mob.setYBodyRot(FACING_CAMERA);
		}
		level.addFreshEntity(entity);
	}

	private static void set(ServerLevel level, int x, int y, int z, BlockState state) {
		level.setBlock(new BlockPos(ORIGIN_X + x, y, ORIGIN_Z + z), state, FLAGS);
	}

	private static int floor(int ground) {
		return ground + World.CROWD_HEIGHT;
	}
}
