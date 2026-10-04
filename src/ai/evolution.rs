use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct PopulationParameters {
    pub population_size: usize,
    pub mutation_rate: f32,
    pub crossover_rate: f32,
}

pub trait PopulationMember: Sized + Clone {
    fn random_new() -> Self;
    fn mutate(&mut self);
    fn crossover(&mut self, other: &Self);
}

pub type FitnessFunction<P> = Box<dyn Fn(&P) -> f32>;

pub enum EvolutionStrategy<P: PopulationMember> {
    GeneticAlgorithm(FitnessFunction<P>),
    Func(Box<dyn Fn(&mut Vec<P>, &PopulationParameters)>),
}

pub enum TournamentSelection {
    TopPercentile(f32),
    TopN(usize),
}
pub enum SelectionStrategy<P: PopulationMember> {
    Tournament(TournamentSelection),
    Func(Box<dyn Fn(&mut Vec<P>, &PopulationParameters)>),
}

pub struct Strategy<P: PopulationMember> {
    pub evolution_strategy: EvolutionStrategy<P>,
    pub selection_strategy: SelectionStrategy<P>,
}

#[derive(Debug, Clone)]
pub struct Population<P: PopulationMember> {
    parameters: PopulationParameters,
    population: Vec<P>,
}

impl<P: PopulationMember> Population<P> {
    pub fn new(parameters: PopulationParameters, gen_func: Option<Box<dyn Fn() -> P>>) -> Self {
        let gen_func = gen_func.unwrap_or_else(|| Box::new(P::random_new));
        let population = (0..parameters.population_size).map(|_| gen_func()).collect();
        Population {
            parameters,
            population,
        }
    }
    pub fn run(&mut self, generations: usize, strategy: &Strategy<P>) {
        match &strategy.evolution_strategy {
            EvolutionStrategy::GeneticAlgorithm(fit_func) => {
                for _ in 0..generations {
                    self.run_genetic_algorithm(&strategy.selection_strategy, fit_func);
                }
            }
            EvolutionStrategy::Func(func) => {
                func(&mut self.population, &self.parameters);
            }
        }
    }
    fn run_genetic_selection(&mut self, selection_strategy: &SelectionStrategy<P>, fit_func: &FitnessFunction<P>) {
        match selection_strategy {
            SelectionStrategy::Tournament(tournament_selection) => {
                let mut fitness_scores = self.population.iter().map(|member| (fit_func(&member), member)).collect::<Vec<_>>();
                fitness_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                match tournament_selection {
                    TournamentSelection::TopPercentile(percent) => {
                        let cutoff = (self.population.len() as f32 * percent) as usize;
                        fitness_scores.truncate(cutoff);
                    }
                    TournamentSelection::TopN(n) => {
                        let cutoff = *n;
                        fitness_scores.truncate(cutoff);
                    }
                };
                self.population = fitness_scores.into_iter().map(|(_, member)| member.clone()).collect::<Vec<_>>();
            }
            SelectionStrategy::Func(func) => {
                func(&mut self.population, &self.parameters);
            }
        }
    }
    fn run_genetic_crossover(&mut self) {
        for i in 0..self.population.len() {
            if rand::random::<f32>() < self.parameters.crossover_rate {
                let mut other = self.population[i].clone();
                other.crossover(&self.population[i]);
                self.population.push(other);
            }
        }
    }
    fn run_genetic_mutation(&mut self) {
        self.population.iter_mut().for_each(|member| {
            if rand::random::<f32>() < self.parameters.mutation_rate {
                member.mutate();
            }
        });
    }
    fn run_populate(&mut self) {
        if self.population.len() > self.parameters.population_size {
            return;
        }
        for _ in 0..(self.parameters.population_size - self.population.len()) {
            self.population.push(P::random_new());
        }
    }
    fn run_genetic_algorithm(&mut self, selection_strategy: &SelectionStrategy<P>, fit_func: &FitnessFunction<P>) {
        self.run_genetic_crossover();
        self.run_genetic_mutation();
        self.run_populate();
        self.run_genetic_selection(selection_strategy, fit_func);
    }
    pub fn population(&self) -> &Vec<P> {
        &self.population
    }
}
