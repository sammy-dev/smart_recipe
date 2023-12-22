#[macro_use]
extern crate serde;
use candid::{CandidType, Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(CandidType, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Snack,
    Desserts,
}

#[derive(CandidType, Clone, Serialize, Deserialize)]
struct Ingredient {
    name: String,
    quantity: String,
}

#[derive(CandidType, Clone, Serialize, Deserialize)]
struct Recipe {
    id: u64,
    name: String,
    ingredients: Vec<Ingredient>,
    instructions: Vec<String>,
    nutritional_info: Vec<String>,
    meal_type: MealType,
    created_at: u64,
    updated_at: Option<u64>,
}

// Implement traits for Recipe struct (Storable, BoundedStorable)
impl Storable for Recipe {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Recipe {
    const MAX_SIZE: u32 = 1024; // Define a suitable maximum size
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static RECIPE_STORAGE: RefCell<StableBTreeMap<u64, Recipe, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(CandidType, Serialize, Deserialize)]
struct RecipePayload {
    name: String,
    ingredients: Vec<Ingredient>,
    instructions: Vec<String>,
    nutritional_info: Vec<String>,
    meal_type: MealType,
}

#[ic_cdk::query]
fn search_recipe(id: u64) -> Result<Recipe, Error> {
    match _get_recipe(&id) {
        Some(recipe) => Ok(recipe),
        None => Err(Error::NotFound {
            msg: format!("A recipe with id={} not found", id),
        }),
    }
}

#[ic_cdk::query]
fn search_by_meal_type(meal_type: MealType) -> Vec<Recipe> {
    let recipes = RECIPE_STORAGE.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, recipe)| recipe.clone())
            .filter(|recipe| recipe.meal_type == meal_type)
            .collect::<Vec<Recipe>>()
    });

    recipes
}

#[ic_cdk::update]
fn add_recipe(recipe_payload: RecipePayload) -> Option<Recipe> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let ingredients = recipe_payload
        .ingredients
        .iter()
        .map(|ing| Ingredient {
            name: ing.name.clone(),
            quantity: ing.quantity.clone(),
        })
        .collect();

    let recipe = Recipe {
        id,
        name: recipe_payload.name,
        ingredients,
        instructions: recipe_payload.instructions,
        nutritional_info: recipe_payload.nutritional_info,
        meal_type: recipe_payload.meal_type,
        created_at: time(),
        updated_at: None,
    };

    do_insert_recipe(&recipe);
    Some(recipe)
}


#[ic_cdk::update]
fn update_recipe(id: u64, payload: RecipePayload) -> Result<Recipe, Error> {
    match RECIPE_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut recipe) => {
            recipe.name = payload.name;
            recipe.ingredients = payload.ingredients;
            recipe.instructions = payload.instructions;
            recipe.nutritional_info = payload.nutritional_info;
            recipe.meal_type = payload.meal_type;
            recipe.updated_at = Some(time());
            do_insert_recipe(&recipe);
            Ok(recipe)
        }
        None => Err(Error::NotFound {
            msg: format!("Couldn't update a recipe with id={}. Recipe not found", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_recipe(id: u64) -> Result<Recipe, Error> {
    match RECIPE_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(recipe) => Ok(recipe),
        None => Err(Error::NotFound {
            msg: format!("Couldn't delete a recipe with id={}. Recipe not found", id),
        }),
    }
}

#[derive(CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// Helper method to get a recipe by id. Used in get_recipe/update_recipe
fn _get_recipe(id: &u64) -> Option<Recipe> {
    RECIPE_STORAGE.with(|service| service.borrow().get(id))
}

// Helper method to perform recipe insert
fn do_insert_recipe(recipe: &Recipe) {
    RECIPE_STORAGE.with(|service| service.borrow_mut().insert(recipe.id, recipe.clone()));
}

// Candid interface generation
ic_cdk::export_candid!();
