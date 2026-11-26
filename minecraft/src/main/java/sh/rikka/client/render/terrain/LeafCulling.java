package sh.rikka.client.render.terrain;

import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.state.BlockState;

public final class LeafCulling {
	public static final int REACH = 2;

	private LeafCulling() {
	}

	public static boolean hidden(BlockAndTintGetter level, BlockState state, Direction direction, BlockPos neighborPos) {
		if (!(state.getBlock() instanceof LeavesBlock) || !(level.getBlockState(neighborPos).getBlock() instanceof LeavesBlock)) {
			return false;
		}
		BlockState behind = level.getBlockState(neighborPos.relative(direction));
		return behind.getBlock() instanceof LeavesBlock || behind.isSolidRender();
	}
}
