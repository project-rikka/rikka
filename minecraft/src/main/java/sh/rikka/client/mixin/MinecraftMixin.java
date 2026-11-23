package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import java.util.Arrays;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.EntityHitResult;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.animation.AnimationHooks;
import sh.rikka.client.resource.ClientModPackSource;

@Mixin(Minecraft.class)
abstract class MinecraftMixin {
	@ModifyReturnValue(method = "createTitle", at = @At("RETURN"))
	private String rikka$createTitle(String title) {
		return title + " | " + RikkaClient.NAME;
	}

	@ModifyArg(
		method = "<init>",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/server/packs/repository/PackRepository;<init>([Lnet/minecraft/server/packs/repository/RepositorySource;)V"
		)
	)
	private RepositorySource[] rikka$addModPacks(RepositorySource[] sources) {
		RepositorySource[] extended = Arrays.copyOf(sources, sources.length + 1);
		extended[sources.length] = new ClientModPackSource();
		return extended;
	}

	@Inject(method = "tick", at = @At("HEAD"))
	private void rikka$tickPre(CallbackInfo callback) {
		ClientHooks.tickPre();
	}

	@Inject(method = "tick", at = @At("RETURN"))
	private void rikka$tickPost(CallbackInfo callback) {
		ClientHooks.tickPost();
	}

	@Inject(method = "close", at = @At("HEAD"))
	private void rikka$close(CallbackInfo callback) {
		RikkaClient.shutdown();
	}

	@WrapOperation(
		method = "startUseItem",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;interact(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/phys/EntityHitResult;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;"
		)
	)
	private InteractionResult rikka$interact(
		MultiPlayerGameMode gameMode, Player player, Entity entity, EntityHitResult hit, InteractionHand hand, Operation<InteractionResult> original
	) {
		return AnimationHooks.interacted(original.call(gameMode, player, entity, hit, hand));
	}

	@WrapOperation(
		method = "startUseItem",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;useItemOn(Lnet/minecraft/client/player/LocalPlayer;Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/phys/BlockHitResult;)Lnet/minecraft/world/InteractionResult;"
		)
	)
	private InteractionResult rikka$useItemOn(
		MultiPlayerGameMode gameMode, LocalPlayer player, InteractionHand hand, BlockHitResult hit, Operation<InteractionResult> original
	) {
		return AnimationHooks.interacted(original.call(gameMode, player, hand, hit));
	}

	@WrapOperation(
		method = "startUseItem",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;useItem(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;"
		)
	)
	private InteractionResult rikka$useItem(MultiPlayerGameMode gameMode, Player player, InteractionHand hand, Operation<InteractionResult> original) {
		return AnimationHooks.interacted(original.call(gameMode, player, hand));
	}
}
