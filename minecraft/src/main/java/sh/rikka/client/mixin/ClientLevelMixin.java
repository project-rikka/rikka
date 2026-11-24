package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import java.util.function.BooleanSupplier;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.weather.WeatherHooks;

@Mixin(ClientLevel.class)
abstract class ClientLevelMixin {
	@Inject(method = "onChunkLoaded", at = @At("TAIL"))
	private void rikka$chunkLoaded(ChunkPos pos, CallbackInfo callback) {
		ClientHooks.chunkLoaded(pos.x(), pos.z());
	}

	@Inject(method = "sendBlockUpdated", at = @At("HEAD"))
	private void rikka$blockUpdated(BlockPos pos, BlockState old, BlockState current, int updateFlags, CallbackInfo callback) {
		ClientHooks.blockUpdated(pos);
	}

	@Inject(method = "tick", at = @At("HEAD"))
	private void rikka$tickWeather(BooleanSupplier haveTime, CallbackInfo callback) {
		WeatherHooks.tick((ClientLevel) (Object) this);
	}

	@ModifyExpressionValue(
		method = "getPrecipitationAt",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/biome/Biome;getPrecipitationAt(Lnet/minecraft/core/BlockPos;I)Lnet/minecraft/world/level/biome/Biome$Precipitation;")
	)
	private Biome.Precipitation rikka$precipitation(Biome.Precipitation vanilla) {
		return WeatherHooks.precipitation((ClientLevel) (Object) this, vanilla);
	}
}
