use super::*;
use std::collections::HashMap;
use termion::event as keyevent;
use nalgebra::{distance, Vector2};
use tui::{layout::{Direction, Group, Rect, Size}, style::{Color, Style},
          widgets::{Block, Borders, Paragraph, Row, SelectableList, Table, Widget, canvas::Canvas}};

use utils::Point;
use astronomicals::system::System;
use entities::Faction;

lazy_static! {
    /// Color mapping for each faction.
    static ref FACTION_COLORS: HashMap<Faction, Color> = {
        let mut m = HashMap::new();
        m.insert(Faction::Empire, Color::Red);
        m.insert(Faction::Federation, Color::Yellow);
        m.insert(Faction::Cartel, Color::Magenta);
        m.insert(Faction::Independent, Color::LightGreen);
        m
    };

    /// Styling for unselected item.
    static ref DEFAULT_STYLE: Style = Style::default();
}

/// The minimum distance within which the gui will snap to the closest system.
const MIN_SNAP_DIST: f64 = 0.9;

/// Displays the map tab.
pub struct MapTab {
    state: Arc<Game>,
    sender: Sender<Event>,
    search_mode: bool,
    search_str: String,
    selected: Option<Point>,
    route: Option<(u32, Vec<Point>)>,
    cursor: Point,
    map_scale: f64,
}

impl MapTab {
    /// Attempts to find a route to the selected system.
    fn find_route(&mut self) {
        let galaxy = &self.state.galaxy.lock().unwrap();
        let player = &mut self.state.player.lock().unwrap();
        let range = match player.ship() {
            &Some(ref ship) => ship.range(),
            &None => 0.,
        };
        let max_jumps = match player.ship() {
            &Some(ref ship) => ship.fuel(),
            &None => 0,
        };
        // Plan route if possible.
        self.route = galaxy.route(
            &player.location(),
            &self.selected.unwrap(),
            range,
            max_jumps,
        );
    }

    /// Moves the player's location to the selected system.
    fn travel_to_selected(&mut self) {
        let player = &mut self.state.player.lock().unwrap();

        // Only travel if the selected system is the same as the cursor and
        // and the final destination for the route.
        if let Some((ref jumps, ref route)) = self.route {
            if self.selected.is_some() && self.selected.unwrap() == self.cursor
                && self.selected.unwrap() == *route.last().unwrap()
            {
                // TODO: Call player to reduce fuel etc.
                player.set_location(&self.selected.unwrap());
                if let &mut Some(ref mut ship) = player.ship_mut() {
                    // TODO: Should we always reduce fuel with 1 per jump?
                    let new_fuel = ship.fuel() - jumps;
                    ship.set_fuel(new_fuel);
                }
                self.sender.send(Event::Travel);
            }
        }

        // Reset route.
        self.route = None;
    }

    /// Draws the event box in the given terminal and area.
    pub fn draw_search(&self, term: &mut Terminal<MouseBackend>, area: &Rect) {
        let draw_str = match self.search_mode {
            true => self.search_str.as_str(),
            false => "Press '/' to search for a system",
        };
        Paragraph::default()
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Yellow))
            .text(draw_str)
            .render(term, &area);
    }

    /// Draw system ship information for the selected system, if any.
    fn draw_system_info(
        &self,
        player_loc: &Point,
        selected_system: Option<&System>,
        term: &mut Terminal<MouseBackend>,
        area: &Rect,
    ) {
        // Do not draw anything if no system is selected.
        if selected_system.is_none() {
            return;
        }
        let system = selected_system.unwrap();

        let system_data = vec![
            format!("Faction:       {}", system.faction.to_string()),
            format!("State:         {}", system.state.to_string()),
            format!("Security:      {}", system.security.to_string()),
            format!("Reputation:    {}", system.reputation.to_string()),
            format!(
                "Distance:      {:.1} ly",
                distance(player_loc, &system.location)
            ),
            format!("Star mass:     {:.1} M", system.star.mass),
            format!("Star type:     {}", system.star.startype.to_string()),
            format!("Bodies:        {}", system.satelites.len()),
        ];

        Group::default()
            .direction(Direction::Vertical)
            .sizes(&[Size::Fixed(9), Size::Min(1)])
            .render(term, area, |term, chunks| {
                SelectableList::default()
                    .items(&system_data)
                    .block(Block::default().title(&format!("{} System", system.name)))
                    .style(Style::default().fg(Color::Yellow))
                    .render(term, &chunks[0]);
                Table::new(
                    // Prepending empty character to get alignment with list above.
                    [" Planet", "Mass", "Population", "Temperature", "Type"].into_iter(),
                    system.satelites.iter().map(|ref planet| {
                        let style: &Style = &DEFAULT_STYLE;
                        Row::StyledData(
                            vec![
                                format!(" {}", planet.name.clone()),
                                format!("{:.1}", planet.mass),
                                format!("{:.1} M", planet.population),
                                format!("{:.1}", planet.surface_temperature),
                                planet.planet_type.to_string(),
                            ].into_iter(),
                            &style,
                        )
                    }),
                ).block(Block::default().title("Planets"))
                    .header_style(Style::default().fg(Color::Yellow))
                    .widths(&[15, 5, 15, 15, 10])
                    .render(term, &chunks[1]);
            });
    }

    /// Draw the galaxy map.
    fn draw_galaxy_map(
        &self,
        player_loc: &Point,
        systems: &Vec<&System>,
        term: &mut Terminal<MouseBackend>,
        area: &Rect,
    ) {
        // Scale map to not overlap systems.
        let map_scaling = 20. * self.map_scale;
        let (max_x, max_y) = systems.iter().fold((0., 0.), |(x_max, y_max), s| {
            (
                (s.location.coords.x / map_scaling).abs().max(x_max),
                (s.location.coords.y / map_scaling).abs().max(y_max),
            )
        });
        Canvas::default()
            .block(Block::default().title("Systems").borders(Borders::ALL))
            .paint(|ctx| {
                for system in systems.iter() {
                    let color = FACTION_COLORS.get(&system.faction).unwrap().clone();
                    ctx.print(
                        system.location.coords.x,
                        system.location.coords.y,
                        ".",
                        color,
                    );
                }
                // Draw player location.
                ctx.print(player_loc.coords.x, player_loc.coords.y, "X", Color::White);

                // Draw the cursor.
                ctx.print(
                    self.cursor.coords.x,
                    self.cursor.coords.y,
                    "*",
                    Color::Yellow,
                );

                // Draw route if available.
                if let Some((_, ref route)) = self.route {
                    for system in route {
                        ctx.print(system.coords.x, system.coords.y, "X", Color::Yellow);
                    }
                    ctx.print(player_loc.coords.x, player_loc.coords.y, "S", Color::Yellow);
                    ctx.print(
                        route.last().unwrap().coords.x,
                        route.last().unwrap().coords.y,
                        "G",
                        Color::Yellow,
                    );
                }
            })
            .x_bounds([self.cursor.coords.x - max_x, self.cursor.coords.x + max_x])
            .y_bounds([self.cursor.coords.y - max_y, self.cursor.coords.y + max_y])
            .render(term, &area);
    }
}

impl Tab for MapTab {
    /// Creates a map tab.
    fn new(state: Arc<Game>, send_handle: Sender<Event>) -> Box<Self> {
        let cursor = state.player.lock().unwrap().location().clone();
        Box::new(MapTab {
            state: state,
            sender: send_handle,
            selected: Some(cursor.clone()),
            search_mode: false,
            search_str: String::new(),
            route: None,
            cursor: cursor,
            map_scale: 1.,
        })
    }

    /// Returns the title string describing the tab.
    fn title(&self) -> String {
        String::from("Galaxy Map")
    }

    /// Handles the user provided event.
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Input(input) => {
                match input {
                    keyevent::Key::Char(' ') if self.selected.is_some() => {
                        match self.route {
                            Some(_) => self.travel_to_selected(),
                            None => self.find_route(),
                        };
                    }
                    // Start search mode.
                    keyevent::Key::Char('/') => {
                        self.search_mode = true;
                        return;
                    }
                    // Quit search mode.
                    keyevent::Key::Esc => {
                        self.search_str.clear();
                        self.search_mode = false;
                        return;
                    }
                    _ => {}
                };

                if self.search_mode {
                    match input {
                        keyevent::Key::Char('\n') => {
                            let galaxy = self.state.galaxy.lock().unwrap();

                            // Set cursor to the closest matching system if
                            // possible.
                            match galaxy.search_name(&self.search_str) {
                                Some(system) => self.cursor = system.location.clone(),
                                None => {}
                            };

                            // Clear input.
                            self.search_str.clear();
                            self.search_mode = false;
                        }
                        keyevent::Key::Char(e) => {
                            self.search_str.push(e);
                            // Early exit.
                            return;
                        }
                        keyevent::Key::Backspace => {
                            self.search_str.pop();
                        }
                        _ => {}
                    };
                }

                self.map_scale *= match input {
                    // Zoom out.
                    keyevent::Key::Char('u') => 0.5,
                    // Zoom in.
                    keyevent::Key::Char('i') => 2.,
                    // No zooming.
                    _ => 1.,
                };

                // Prevent zooming too far in.
                self.map_scale = self.map_scale.min(4.);

                let translation = match input {
                    // Move up.
                    keyevent::Key::Char('k') => Vector2::new(0., 1. / self.map_scale),
                    // Move down.
                    keyevent::Key::Char('j') => Vector2::new(0., -1. / self.map_scale),
                    // Move right.
                    keyevent::Key::Char('l') => Vector2::new(1. / self.map_scale, 0.),
                    // Move left.
                    keyevent::Key::Char('h') => Vector2::new(-1. / self.map_scale, 0.),
                    _ => Vector2::new(0., 0.),
                };

                // Move out of snapping system if currently snapped.
                self.cursor += match self.selected {
                    Some(_) => Vector2::new(
                        translation.x * 1.1 * self.map_scale * MIN_SNAP_DIST,
                        translation.y * 1.1 * self.map_scale * MIN_SNAP_DIST,
                    ),
                    None => translation,
                };
                self.selected = None;

                // Check if cursor should snap to closest system.
                if let Some(neighbor) = self.state.galaxy.lock().unwrap().nearest(&self.cursor) {
                    if distance(&self.cursor, &neighbor) < MIN_SNAP_DIST {
                        self.cursor = neighbor.clone();
                        self.selected = Some(neighbor.clone());
                    }
                }
            }
            _ => {}
        };
    }

    /// Draws the tab in the given terminal and area.
    fn draw(&self, term: &mut Terminal<MouseBackend>, area: &Rect) {
        let galaxy = self.state.galaxy.lock().unwrap();
        Group::default()
            .direction(Direction::Horizontal)
            .sizes(&[Size::Fixed(75), Size::Min(1)])
            .render(term, area, |term, chunks| {
                // TODO: Draw system detailed information.
                let systems = &galaxy.systems();
                let player_loc = &self.state.player.lock().unwrap().location().clone();
                // Draw sidebar.
                Group::default()
                    .direction(Direction::Vertical)
                    .sizes(&[Size::Min(1), Size::Fixed(3)])
                    .render(term, &chunks[0], |term, sidebar_chunk| {
                        self.draw_system_info(
                            &player_loc,
                            self.selected.map(|point| galaxy.system(&point).unwrap()),
                            term,
                            &sidebar_chunk[0],
                        );
                        self.draw_search(term, &sidebar_chunk[1]);
                    });
                self.draw_galaxy_map(&player_loc, &systems, term, &chunks[1]);
            });
    }
}
