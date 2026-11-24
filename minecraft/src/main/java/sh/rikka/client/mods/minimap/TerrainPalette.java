package sh.rikka.client.mods.minimap;

import com.mojang.blaze3d.platform.NativeImage;
import it.unimi.dsi.fastutil.objects.Reference2ObjectOpenHashMap;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.color.block.BlockTintSource;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.block.FluidModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.util.ARGB;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.MapColor;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.SpriteContentsAccessor;

final class TerrainPalette {
	private static final long MODEL_SEED = 42L;
	private static final int NO_COLOR = 0;

	private final Reference2ObjectOpenHashMap<BlockState, Sample> samples = new Reference2ObjectOpenHashMap<>();
	private final List<BlockStateModelPart> parts = new ArrayList<>();
	private final RandomSource random = RandomSource.create(MODEL_SEED);

	void clear() {
		this.samples.clear();
	}

	int color(TerrainColors colors, ClientLevel level, BlockState state, BlockPos pos) {
		return switch (colors) {
			case MAP -> state.getMapColor(level, pos).calculateARGBColor(MapColor.Brightness.NORMAL);
			case BLOCKS -> blockColor(level, state, pos);
		};
	}

	private int blockColor(ClientLevel level, BlockState state, BlockPos pos) {
		Sample sample = this.samples.get(state);
		if (sample == null) {
			sample = sample(state, level, pos);
			this.samples.put(state, sample);
		}
		BlockTintSource tint = sample.tint();
		if (tint == null) {
			return sample.argb();
		}
		return ARGB.multiply(sample.argb(), ARGB.opaque(tint.colorInWorld(state, level, pos)));
	}

	private Sample sample(BlockState state, ClientLevel level, BlockPos pos) {
		Minecraft minecraft = Minecraft.getInstance();
		FluidState fluid = state.getFluidState();
		if (state.getBlock() instanceof LiquidBlock && !fluid.isEmpty()) {
			FluidModel model = minecraft.getModelManager().getFluidStateModelSet().get(fluid);
			return withFallback(average(model.stillMaterial().sprite()), model.tintSource(), state, level, pos);
		}
		BlockStateModel model = minecraft.getModelManager().getBlockStateModelSet().get(state);
		this.parts.clear();
		this.random.setSeed(MODEL_SEED);
		model.collectParts(this.random, this.parts);
		BakedQuad quad = firstQuad(Direction.UP);
		if (quad == null) {
			quad = firstQuad(null);
		}
		if (quad == null) {
			return withFallback(average(model.particleMaterial().sprite()), null, state, level, pos);
		}
		BakedQuad.MaterialInfo material = quad.materialInfo();
		BlockTintSource tint = material.tintIndex() >= 0 ? minecraft.getBlockColors().getTintSource(state, material.tintIndex()) : null;
		return withFallback(average(material.sprite()), tint, state, level, pos);
	}

	private @Nullable BakedQuad firstQuad(@Nullable Direction direction) {
		for (BlockStateModelPart part : this.parts) {
			List<BakedQuad> quads = part.getQuads(direction);
			if (!quads.isEmpty()) {
				return quads.getFirst();
			}
		}
		return null;
	}

	private static Sample withFallback(int argb, @Nullable BlockTintSource tint, BlockState state, ClientLevel level, BlockPos pos) {
		if (argb != NO_COLOR) {
			return new Sample(argb, tint);
		}
		return new Sample(state.getMapColor(level, pos).calculateARGBColor(MapColor.Brightness.NORMAL), null);
	}

	private static int average(TextureAtlasSprite sprite) {
		SpriteContents contents = sprite.contents();
		NativeImage image = ((SpriteContentsAccessor) contents).rikka$originalImage();
		int width = Math.min(contents.width(), image.getWidth());
		int height = Math.min(contents.height(), image.getHeight());
		long red = 0;
		long green = 0;
		long blue = 0;
		long weight = 0;
		for (int y = 0; y < height; y++) {
			for (int x = 0; x < width; x++) {
				int pixel = image.getPixel(x, y);
				int alpha = ARGB.alpha(pixel);
				red += (long) ARGB.red(pixel) * alpha;
				green += (long) ARGB.green(pixel) * alpha;
				blue += (long) ARGB.blue(pixel) * alpha;
				weight += alpha;
			}
		}
		if (weight == 0) {
			return NO_COLOR;
		}
		return ARGB.color((int) (red / weight), (int) (green / weight), (int) (blue / weight));
	}

	private record Sample(int argb, @Nullable BlockTintSource tint) {
	}
}
