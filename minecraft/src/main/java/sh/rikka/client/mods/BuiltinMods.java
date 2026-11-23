package sh.rikka.client.mods;

import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.mods.animation.OldAnimationsMod;
import sh.rikka.client.mods.camera.FreeLookMod;
import sh.rikka.client.mods.camera.TapLookMod;
import sh.rikka.client.mods.camera.ZoomMod;
import sh.rikka.client.mods.hud.ClockMod;
import sh.rikka.client.mods.hud.ComboCounterMod;
import sh.rikka.client.mods.hud.CoordsMod;
import sh.rikka.client.mods.hud.CpsDisplayMod;
import sh.rikka.client.mods.hud.DayCounterMod;
import sh.rikka.client.mods.hud.FpsDisplayMod;
import sh.rikka.client.mods.hud.GameModeDisplayMod;
import sh.rikka.client.mods.hud.HealthDisplayMod;
import sh.rikka.client.mods.hud.JumpResetIndicatorMod;
import sh.rikka.client.mods.hud.MemoryUsageMod;
import sh.rikka.client.mods.hud.NameDisplayMod;
import sh.rikka.client.mods.hud.PingDisplayMod;
import sh.rikka.client.mods.hud.PlayerCounterMod;
import sh.rikka.client.mods.hud.ReachDisplayMod;
import sh.rikka.client.mods.hud.ServerAddressMod;
import sh.rikka.client.mods.hud.SpeedometerMod;
import sh.rikka.client.mods.hud.StopwatchMod;
import sh.rikka.client.mods.hud.WeatherDisplayMod;
import sh.rikka.client.mods.mechanic.HitDelayFixMod;
import sh.rikka.client.mods.mechanic.NoJumpDelayMod;
import sh.rikka.client.mods.mechanic.SnapTapMod;
import sh.rikka.client.mods.visual.BloodParticleMod;
import sh.rikka.client.mods.visual.ForceMainHandMod;
import sh.rikka.client.mods.visual.FullbrightMod;
import sh.rikka.client.mods.visual.ParticlesMod;
import sh.rikka.client.mods.visual.ProjectileTrailMod;
import sh.rikka.client.setting.Anchor;
import sh.rikka.client.setting.ScreenPosition;

public final class BuiltinMods {
	private static final double MARGIN = 4.0;
	private static final double ROW_HEIGHT = SimpleHudMod.HEIGHT + MARGIN;

	private BuiltinMods() {
	}

	public static void register(ModManager mods) {
		mods.register(new OldAnimationsMod());
		mods.register(new ForceMainHandMod());
		mods.register(new FullbrightMod());
		mods.register(new ParticlesMod());
		mods.register(new BloodParticleMod());
		mods.register(new ProjectileTrailMod());
		mods.register(new FreeLookMod());
		mods.register(new TapLookMod());
		mods.register(new ZoomMod());
		mods.register(new HitDelayFixMod());
		mods.register(new NoJumpDelayMod());
		mods.register(new SnapTapMod());
		mods.register(new FpsDisplayMod(left(0)));
		mods.register(new PingDisplayMod(left(1)));
		mods.register(new CpsDisplayMod(left(2)));
		mods.register(new ReachDisplayMod(left(3)));
		mods.register(new ComboCounterMod(left(4)));
		mods.register(new SpeedometerMod(left(5)));
		mods.register(new JumpResetIndicatorMod(left(6)));
		mods.register(new CoordsMod(left(7)));
		mods.register(new ClockMod(right(0)));
		mods.register(new DayCounterMod(right(1)));
		mods.register(new WeatherDisplayMod(right(2)));
		mods.register(new PlayerCounterMod(right(3)));
		mods.register(new ServerAddressMod(right(4)));
		mods.register(new StopwatchMod(right(5)));
		mods.register(new MemoryUsageMod(right(6)));
		mods.register(new GameModeDisplayMod(right(7)));
		mods.register(new NameDisplayMod(right(8)));
		mods.register(new HealthDisplayMod(right(9)));
	}

	private static ScreenPosition left(int row) {
		return new ScreenPosition(Anchor.TOP_LEFT, MARGIN, MARGIN + row * ROW_HEIGHT);
	}

	private static ScreenPosition right(int row) {
		return new ScreenPosition(Anchor.TOP_RIGHT, -MARGIN, MARGIN + row * ROW_HEIGHT);
	}
}
