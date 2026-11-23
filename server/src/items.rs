use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Cấu hình khuôn mẫu vật phẩm từ data/items.json
#[derive(Debug, Clone, Deserialize)]
pub struct ItemTemplate {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub heal_amount: i32,
    pub max_stack: u32,
    pub description: String,
}

/// Sổ đăng ký danh mục vật phẩm trong game
pub struct ItemRegistry {
    items: HashMap<String, ItemTemplate>,
}

impl ItemRegistry {
    pub fn load_from_data_or_defaults() -> Self {
        let path = Path::new("data/items.json");
        let list: Vec<ItemTemplate> = if path.exists() {
            fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(Self::defaults)
        } else {
            Self::defaults()
        };

        let mut items = HashMap::new();
        for item in list {
            items.insert(item.id.clone(), item);
        }

        Self { items }
    }

    pub fn get(&self, id: &str) -> Option<&ItemTemplate> {
        self.items.get(id)
    }

    fn defaults() -> Vec<ItemTemplate> {
        vec![
            ItemTemplate {
                id: "potion_hp".to_string(),
                name: "Bình máu nhỏ".to_string(),
                item_type: "consumable".to_string(),
                heal_amount: 40,
                max_stack: 20,
                description: "Hồi phục 40 điểm sinh lực khi sử dụng.".to_string(),
            },
            ItemTemplate {
                id: "goblin_ear".to_string(),
                name: "Tai yêu tinh".to_string(),
                item_type: "material".to_string(),
                heal_amount: 0,
                max_stack: 99,
                description: "Chiến lợi phẩm từ yêu tinh, có thể bán hoặc dùng làm nhiệm vụ."
                    .to_string(),
            },
            ItemTemplate {
                id: "wolf_pelt".to_string(),
                name: "Da sói xám".to_string(),
                item_type: "material".to_string(),
                heal_amount: 0,
                max_stack: 99,
                description: "Bộ da sói dày, có thể bán hoặc dùng chế tạo áo giáp.".to_string(),
            },
        ]
    }
}
