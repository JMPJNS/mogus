import LdtkProject;

class Main extends hxd.App {
	static function main() {
		// Boot
		new Main();
	}

	override function init() {
		super.init();

		// Init general heaps stuff
		hxd.Res.initEmbed();
		s2d.setScale(dn.heaps.Scaler.bestFit_i(256, 256)); // scale view to fit

		// Read project JSON
		var project = new LdtkProject();

		// Render each level
		for (level in project.levels) {
			// Create a wrapper to render all layers in it
			var levelWrapper = new h2d.Object(s2d);

			// Position accordingly to world pixel coords
			levelWrapper.x = level.worldX;
			levelWrapper.y = level.worldY;

			// Level background image
			if (level.hasBgImage())
				levelWrapper.addChild(level.getBgBitmap());

			// Render background layer
			levelWrapper.addChild(level.l_Tiles.render());
		}
	}
}
