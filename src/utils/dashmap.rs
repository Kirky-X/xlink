//! DashMap 辅助函数
//!
//! 提供通用的 DashMap 操作函数

use dashmap::DashMap;
use std::hash::Hash;

/// 清理 DashMap 并返回所有键
///
/// # 参数
///
/// * `map` - 要清理的 DashMap
///
/// # 返回
///
/// 返回所有键的列表
///
/// # 示例
///
/// ```ignore
/// use dashmap::DashMap;
/// use xpush::utils::clear_dashmap;
///
/// let map: DashMap<u32, String> = DashMap::new();
/// map.insert(1, "a".to_string());
/// map.insert(2, "b".to_string());
///
/// let keys = clear_dashmap(&map);
/// assert_eq!(keys.len(), 2);
/// assert!(map.is_empty());
/// ```
pub fn clear_dashmap<K, V>(map: &DashMap<K, V>) -> Vec<K>
where
    K: Clone + Eq + Hash,
{
    let keys: Vec<_> = map.iter().map(|entry| entry.key().clone()).collect();
    for key in &keys {
        map.remove(key);
    }
    keys
}

/// 批量删除 DashMap 中的键
///
/// # 参数
///
/// * `map` - 要操作的 DashMap
/// * `keys` - 要删除的键列表
///
/// # 示例
///
/// ```ignore
/// use dashmap::DashMap;
/// use xpush::utils::remove_keys;
///
/// let map: DashMap<u32, String> = DashMap::new();
/// map.insert(1, "a".to_string());
/// map.insert(2, "b".to_string());
/// map.insert(3, "c".to_string());
///
/// remove_keys(&map, vec![1, 3]);
/// assert_eq!(map.len(), 1);
/// assert!(map.contains_key(&2));
/// ```
pub fn remove_keys<K, V>(map: &DashMap<K, V>, keys: Vec<K>)
where
    K: Eq + Hash,
{
    for key in keys {
        map.remove(&key);
    }
}

/// 获取 DashMap 中的所有键
///
/// # 参数
///
/// * `map` - 要操作的 DashMap
///
/// # 返回
///
/// 返回所有键的列表
///
/// # 示例
///
/// ```ignore
/// use dashmap::DashMap;
/// use xpush::utils::get_all_keys;
///
/// let map: DashMap<u32, String> = DashMap::new();
/// map.insert(1, "a".to_string());
/// map.insert(2, "b".to_string());
///
/// let keys = get_all_keys(&map);
/// assert_eq!(keys.len(), 2);
/// ```
pub fn get_all_keys<K, V>(map: &DashMap<K, V>) -> Vec<K>
where
    K: Clone + Eq + Hash,
{
    map.iter().map(|entry| entry.key().clone()).collect()
}

/// 获取 DashMap 中的所有值
///
/// # 参数
///
/// * `map` - 要操作的 DashMap
///
/// # 返回
///
/// 返回所有值的列表
///
/// # 示例
///
/// ```ignore
/// use dashmap::DashMap;
/// use xpush::utils::get_all_values;
///
/// let map: DashMap<u32, String> = DashMap::new();
/// map.insert(1, "a".to_string());
/// map.insert(2, "b".to_string());
///
/// let values = get_all_values(&map);
/// assert_eq!(values.len(), 2);
/// ```
pub fn get_all_values<K, V>(map: &DashMap<K, V>) -> Vec<V>
where
    K: Eq + Hash,
    V: Clone,
{
    map.iter().map(|entry| entry.value().clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_dashmap() {
        let map: DashMap<u32, String> = DashMap::new();
        map.insert(1, "a".to_string());
        map.insert(2, "b".to_string());

        let keys = clear_dashmap(&map);
        assert_eq!(keys.len(), 2);
        assert!(map.is_empty());
    }

    #[test]
    fn test_remove_keys() {
        let map: DashMap<u32, String> = DashMap::new();
        map.insert(1, "a".to_string());
        map.insert(2, "b".to_string());
        map.insert(3, "c".to_string());

        remove_keys(&map, vec![1, 3]);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&2));
    }

    #[test]
    fn test_get_all_keys() {
        let map: DashMap<u32, String> = DashMap::new();
        map.insert(1, "a".to_string());
        map.insert(2, "b".to_string());

        let keys = get_all_keys(&map);
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_get_all_values() {
        let map: DashMap<u32, String> = DashMap::new();
        map.insert(1, "a".to_string());
        map.insert(2, "b".to_string());

        let values = get_all_values(&map);
        assert_eq!(values.len(), 2);
    }
}
