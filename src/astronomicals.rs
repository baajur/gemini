use nalgebra::geometry::Point3;
use rand::{Rng, SeedableRng};
use rand::isaac::Isaac64Rng;
use std::sync::{Arc, Mutex};
use std::f64::consts::PI;
use statrs::distribution::{Gamma, Distribution};

use generators::stars::StarGen;
use generators::names::NameGen;
use generators::planets::PlanetGen;
use generators::MutGen;
use generators::Gen;

#[derive(Debug)]
pub struct Galaxy {
    systems: Vec<System>,
}

impl Galaxy {
    pub fn new(systems: Vec<System>) -> Self {
        Galaxy { systems }
    }
}

#[derive(Debug)]
pub struct Star {
    mass: f64,
    luminosity: f64,
    metalicity: f64,
}

impl Star {
    pub fn new(mass: f64, luminosity: f64, metalicity: f64) -> Self {
        Star {
            mass,
            luminosity,
            metalicity,
        }
    }
}

#[derive(Debug)]
pub struct Planet {
    name: String,
    mass: f64,
    gravity: f64,
    orbit_distance: f64,
    surface_temperature: f64,
    planet_type: PlanetType,
}

#[derive(Debug)]
enum PlanetType {
    Metal_rich,
    Icy,
    Rocky,
    Gas_giant,
    Earth_like,
    Water,
    Water_giant,
}

impl Planet {
    pub fn new(mass: f64, orbit_distance: f64) -> Self {

        // TODO: Make something a bit more accurate
        let gravity = mass;
        Planet {
            mass,
            gravity,
            orbit_distance,
            name: String::new(),
            surface_temperature: 0.,
            planet_type: PlanetType::Rocky,
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_surface_temperature(&mut self, star: &Star) {
        self.surface_temperature = (star.luminosity * 3.846 * 10f64.powi(26) * (1. - 0.29) /
                                        (16. * PI * (299692458. * self.orbit_distance).powi(2) *
                                             5.670373 *
                                             10f64.powi(-8)))
            .powf(0.25);
    }
}

#[derive(Debug)]
pub struct System {
    location: Point3<f64>,
    name: String,
    star: Star,
    pub satelites: Vec<Planet>,
}

impl System {
    pub fn new(
        location: Point3<f64>,
        name_gen: Arc<Mutex<NameGen>>,
        star_gen: &StarGen,
        planet_gen: &PlanetGen,
    ) -> Self {

        // Calculate hash
        let hash = System::hash(location);
        let seed: &[_] = &[hash];
        let mut rng: Isaac64Rng = SeedableRng::from_seed(seed);

        let star = star_gen.generate(&mut rng).unwrap();

        // Unwrap and lock name generator as it is mutated by generation
        let mut name_gen_unwraped = name_gen.lock().unwrap();


        // TODO: Replace constant in config
        let num_planets = Gamma::new(1., 0.5)
            .unwrap()
            .sample::<Isaac64Rng>(&mut rng)
            .round() as u32;
        let mut satelites: Vec<Planet> = (0..num_planets)
            .map(|_| planet_gen.generate(&mut rng).unwrap())
            .collect();

        // Fallback to "Unnamed" for names
        for planet in &mut satelites {
            planet.set_name(name_gen_unwraped.generate().unwrap_or(
                String::from("Unnamed"),
            ));
            planet.set_surface_temperature(&star);
        }

        // System name is the same as one random planet
        let name = match rng.choose(&satelites) {
            Some(planet) => planet.name.clone(),
            None => {
                name_gen_unwraped.generate().unwrap_or(
                    String::from("Unnamed"),
                )
            }
        } + " System";

        System {
            location,
            name,
            star,
            satelites,
        }
    }

    /// Hash based on location, algorithm used is presented in the paper:
    /// Optimized Spatial Hashing for Collision Detection of Deformable Objects
    fn hash(location: Point3<f64>) -> u64 {
        let values = location
            .iter()
            .zip(&[73856093f64, 19349663f64, 83492791f64])
            .map(|(&a, &b)| (a * b) as u64)
            .collect::<Vec<_>>();
        values.iter().fold(0, |acc, &val| acc ^ val)
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::isaac::Isaac64Rng;
    use super::*;
    extern crate env_logger;
    use statrs::distribution::{Distribution, Uniform};
    use std::collections::HashMap;

    #[test]
    fn test_hash_uniqueness() {
        let _ = env_logger::init();

        let new_seed: &[_] = &[42 as u64];
        let mut rng: Isaac64Rng = SeedableRng::from_seed(new_seed);
        let n = Uniform::new(0., 100000.).unwrap();

        let mut hashes = HashMap::new();
        let tries = 10000;
        for _ in 0..tries {
            let loc = Point3::new(
                n.sample::<Isaac64Rng>(&mut rng),
                n.sample::<Isaac64Rng>(&mut rng),
                n.sample::<Isaac64Rng>(&mut rng),
            );
            hashes.insert(System::hash(loc), loc);
        }
        assert_eq!(hashes.len(), tries);

    }
}
