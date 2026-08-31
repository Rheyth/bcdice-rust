use bcdice::eval::eval_command;
use bcdice::game_system::GameSystemId;
use bcdice::randomizer::SeededRandomizer;

fn main() {
    let system = GameSystemId::new("DiceBot");
    let mut rng = SeededRandomizer::new(vec![(3, 6), (5, 6)]);
    let r = eval_command(&system, "2D6", &mut rng).unwrap();
    println!("{}", r.map(|x| x.text).unwrap_or_default());
}
