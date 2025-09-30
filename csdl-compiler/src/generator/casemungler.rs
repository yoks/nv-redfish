// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[must_use]
pub fn camel_to_snake<S>(camel_str: S) -> String
where
    S: AsRef<str>,
{
    camel_to_words(camel_str.as_ref())
        .collect::<Vec<String>>()
        .join("_")
        .to_lowercase()
}

fn camel_to_words(s: &str) -> impl Iterator<Item = String> {
    let chars: Vec<char> = s.chars().collect();

    chars
        .iter()
        .enumerate()
        .fold(vec![vec![]], |mut words: Vec<Vec<char>>, (i, &ch)| {
            // catch all situations where we need to separate stream of chars into words
            if i > 0 && ch.is_uppercase() && {
                let prev_char = chars[i - 1];

                // case 1: new word: transition from lower to uppercase (standard camelCase)
                prev_char.is_lowercase() ||
                    // case 2: new word: transition from an uppercase acronym letter to lowercase
                    (prev_char.is_uppercase() &&
                        i + 1 < chars.len() && chars[i + 1].is_lowercase() &&
                        // assume that the following 2+ lowercase letters are a new word 
                        chars[(i + 1)..]
                            .iter()
                            .take_while(|&&c| c.is_lowercase())
                            .count() >= 2)
            } {
                words.push(vec![]);
            }

            if let Some(curr_word) = words.last_mut() {
                curr_word.push(ch);
            }
            words
        })
        .into_iter()
        .map(|w| w.into_iter().collect::<String>())
        .collect::<Vec<String>>()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_casemungler_camel2snake_with_string() {
        let owned_string = String::from("CamelCase");
        assert_eq!(camel_to_snake(owned_string), "camel_case");
    }

    #[test]
    fn test_casemungler_camel2snake_with_str() {
        let with_str = "CamelCase";
        assert_eq!(camel_to_snake(with_str), "camel_case");
    }

    #[test]
    fn test_casemungler_camel2snake_normal() {
        assert_eq!(
            camel_to_snake("PhysicalFunctionNumber"),
            "physical_function_number"
        );
        assert_eq!(
            camel_to_snake("physicalFunctionNumber"),
            "physical_function_number"
        );
    }

    #[test]
    fn test_casemungler_camel2snake_empty_string() {
        assert_eq!(camel_to_snake(""), "");
    }

    #[test]
    fn test_casemungler_camel2snake_single_char() {
        assert_eq!(camel_to_snake("F"), "f");
    }

    #[test]
    fn test_casemungler_camel2snake_special_first_char() {
        assert_eq!(camel_to_snake("_"), "_");
        assert_eq!(camel_to_snake("_SomeThing"), "_some_thing");
    }

    #[test]
    fn test_casemungler_camel2snake_simple_cases() {
        assert_eq!(camel_to_snake("Pf"), "pf");
        assert_eq!(camel_to_snake("pF"), "p_f");
    }

    #[test]
    fn test_casemungler_camel2snake_acronyms() {
        assert_eq!(camel_to_snake("NVMe"), "nvme");
        assert_eq!(camel_to_snake("NVME"), "nvme");
        assert_eq!(camel_to_snake("nVME"), "n_vme");
    }

    #[test]
    fn test_casemungler_camel2snake_acronym_with_words() {
        assert_eq!(camel_to_snake("nVMEfoobar"), "n_vm_efoobar");
        assert_eq!(camel_to_snake("nVMEFoobar"), "n_vme_foobar");
        assert_eq!(camel_to_snake("PCIEFunctions"), "pcie_functions");
        assert_eq!(camel_to_snake("PFFunctionNumber"), "pf_function_number");
    }
}
