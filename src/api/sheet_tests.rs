//! Comprehensive tests for the Sheet trait implementation using TestSheet.

use super::{Sheet, TestSheet, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use std::collections::HashMap;

#[tokio::test]
async fn test_get_transactions_default_data() {
    let mut sheet = TestSheet::default();
    let result = sheet.get(TRANSACTIONS).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert!(!data.is_empty(), "Transactions sheet should not be empty");
    assert!(data.len() > 1, "Should have header + data rows");

    // Verify header row exists and has expected columns
    let header = &data[0];
    assert!(header.contains(&"Transaction ID".to_string()));
    assert!(header.contains(&"Date".to_string()));
    assert!(header.contains(&"Description".to_string()));
    assert!(header.contains(&"Amount".to_string()));
}

#[tokio::test]
async fn test_get_categories_default_data() {
    let mut sheet = TestSheet::default();
    let result = sheet.get(CATEGORIES).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert!(!data.is_empty(), "Categories sheet should not be empty");

    // Verify header row
    let header = &data[0];
    assert!(header.contains(&"Category".to_string()));
    assert!(header.contains(&"Group".to_string()));
    assert!(header.contains(&"Type".to_string()));
}

#[tokio::test]
async fn test_get_autocat_default_data() {
    let mut sheet = TestSheet::default();
    let result = sheet.get(AUTO_CAT).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert!(!data.is_empty(), "AutoCat sheet should not be empty");

    // Verify header row
    let header = &data[0];
    assert!(header.contains(&"Category".to_string()));
    assert!(header.contains(&"Description Contains".to_string()));
}

#[tokio::test]
async fn test_get_nonexistent_sheet() {
    let mut sheet = TestSheet::default();
    let result = sheet.get("NonexistentSheet").await;
    assert!(
        result.is_err(),
        "Should error when getting nonexistent sheet"
    );

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("NonexistentSheet"));
    assert!(error_msg.contains("not found"));
}

#[tokio::test]
async fn test_put_and_get_round_trip() {
    let mut sheet = TestSheet::default();

    let test_data = vec![
        vec![
            "Header1".to_string(),
            "Header2".to_string(),
            "Header3".to_string(),
        ],
        vec![
            "Row1Col1".to_string(),
            "Row1Col2".to_string(),
            "Row1Col3".to_string(),
        ],
        vec![
            "Row2Col1".to_string(),
            "Row2Col2".to_string(),
            "Row2Col3".to_string(),
        ],
    ];

    // Put data
    let put_result = sheet._put("TestSheet", &test_data).await;
    assert!(put_result.is_ok(), "Put should succeed");

    // Get it back
    let get_result = sheet.get("TestSheet").await;
    assert!(get_result.is_ok(), "Get should succeed after put");

    let retrieved_data = get_result.unwrap();
    assert_eq!(
        retrieved_data, test_data,
        "Retrieved data should match what was put"
    );
}

#[tokio::test]
async fn test_put_replaces_existing_data() {
    let mut sheet = TestSheet::default();

    // Get original transaction count
    let original = sheet.get(TRANSACTIONS).await.unwrap();
    let original_len = original.len();

    // Put new data with different number of rows
    let new_data = vec![
        vec!["Col1".to_string(), "Col2".to_string()],
        vec!["Data1".to_string(), "Data2".to_string()],
    ];

    sheet._put(TRANSACTIONS, &new_data).await.unwrap();

    // Verify replacement
    let replaced = sheet.get(TRANSACTIONS).await.unwrap();
    assert_eq!(replaced.len(), 2, "Should have only 2 rows now");
    assert_ne!(replaced.len(), original_len, "Length should have changed");
    assert_eq!(replaced, new_data);
}

#[tokio::test]
async fn test_put_empty_data() {
    let mut sheet = TestSheet::default();

    let empty_data: Vec<Vec<String>> = vec![];
    let result = sheet._put("EmptySheet", &empty_data).await;
    assert!(result.is_ok(), "Should allow putting empty data");

    // Verify it was stored
    let retrieved = sheet.get("EmptySheet").await.unwrap();
    assert_eq!(retrieved.len(), 0, "Should retrieve empty data");
}

#[tokio::test]
async fn test_put_single_row() {
    let mut sheet = TestSheet::default();

    let single_row = vec![vec![
        "Only".to_string(),
        "One".to_string(),
        "Row".to_string(),
    ]];

    sheet._put("SingleRow", &single_row).await.unwrap();
    let retrieved = sheet.get("SingleRow").await.unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].len(), 3);
}

#[tokio::test]
async fn test_put_with_varying_column_counts() {
    let mut sheet = TestSheet::default();

    // Rows with different numbers of columns (this should be allowed)
    let jagged_data = vec![
        vec!["H1".to_string(), "H2".to_string(), "H3".to_string()],
        vec!["R1C1".to_string(), "R1C2".to_string()], // Only 2 columns
        vec!["R2C1".to_string()],                     // Only 1 column
        vec![
            "R3C1".to_string(),
            "R3C2".to_string(),
            "R3C3".to_string(),
            "R3C4".to_string(),
        ], // 4 columns
    ];

    sheet._put("JaggedSheet", &jagged_data).await.unwrap();
    let retrieved = sheet.get("JaggedSheet").await.unwrap();
    assert_eq!(retrieved, jagged_data);
}

#[tokio::test]
async fn test_put_with_empty_strings() {
    let mut sheet = TestSheet::default();

    let data_with_empties = vec![
        vec!["A".to_string(), "".to_string(), "C".to_string()],
        vec!["".to_string(), "B".to_string(), "".to_string()],
        vec!["".to_string(), "".to_string(), "".to_string()],
    ];

    sheet
        ._put("EmptyStrings", &data_with_empties)
        .await
        .unwrap();
    let retrieved = sheet.get("EmptyStrings").await.unwrap();
    assert_eq!(retrieved, data_with_empties);
}

#[tokio::test]
async fn test_put_with_special_characters() {
    let mut sheet = TestSheet::default();

    let special_chars = vec![
        vec![
            "Header".to_string(),
            "With,Comma".to_string(),
            "With\"Quote".to_string(),
        ],
        vec![
            "Line\nBreak".to_string(),
            "Tab\there".to_string(),
            "Multi\r\nLine".to_string(),
        ],
        vec![
            "Emoji😀".to_string(),
            "Ñoño".to_string(),
            "日本語".to_string(),
        ],
    ];

    sheet._put("SpecialChars", &special_chars).await.unwrap();
    let retrieved = sheet.get("SpecialChars").await.unwrap();
    assert_eq!(retrieved, special_chars);
}

#[tokio::test]
async fn test_multiple_sheets_isolation() {
    let mut sheet = TestSheet::default();

    let sheet1_data = vec![vec!["Sheet1".to_string()]];
    let sheet2_data = vec![vec!["Sheet2".to_string()]];
    let sheet3_data = vec![vec!["Sheet3".to_string()]];

    sheet._put("Sheet1", &sheet1_data).await.unwrap();
    sheet._put("Sheet2", &sheet2_data).await.unwrap();
    sheet._put("Sheet3", &sheet3_data).await.unwrap();

    // Verify each sheet has its own data
    assert_eq!(sheet.get("Sheet1").await.unwrap(), sheet1_data);
    assert_eq!(sheet.get("Sheet2").await.unwrap(), sheet2_data);
    assert_eq!(sheet.get("Sheet3").await.unwrap(), sheet3_data);

    // Modifying one shouldn't affect others
    let new_sheet1 = vec![vec!["Modified".to_string()]];
    sheet._put("Sheet1", &new_sheet1).await.unwrap();

    assert_eq!(sheet.get("Sheet1").await.unwrap(), new_sheet1);
    assert_eq!(sheet.get("Sheet2").await.unwrap(), sheet2_data);
    assert_eq!(sheet.get("Sheet3").await.unwrap(), sheet3_data);
}

#[tokio::test]
async fn test_large_dataset() {
    let mut sheet = TestSheet::default();

    // Create a large dataset (1000 rows, 20 columns)
    let mut large_data = Vec::new();
    large_data.push((0..20).map(|i| format!("Header{i}")).collect::<Vec<_>>());

    for row in 1..=1000 {
        large_data.push(
            (0..20)
                .map(|col| format!("R{row}C{col}"))
                .collect::<Vec<_>>(),
        );
    }

    sheet._put("LargeSheet", &large_data).await.unwrap();
    let retrieved = sheet.get("LargeSheet").await.unwrap();

    assert_eq!(retrieved.len(), 1001); // Header + 1000 rows
    assert_eq!(retrieved[0].len(), 20);
    assert_eq!(retrieved[500][10], "R500C10");
}

#[tokio::test]
async fn test_custom_testsheet_creation() {
    let mut map = HashMap::new();
    map.insert(
        "CustomSheet".to_string(),
        vec![
            vec!["Custom".to_string(), "Data".to_string()],
            vec!["Row1".to_string(), "Value1".to_string()],
        ],
    );

    let mut sheet = TestSheet::new(map);

    let result = sheet.get("CustomSheet").await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0][0], "Custom");

    // Should not have default sheets
    assert!(sheet.get(TRANSACTIONS).await.is_err());
}

#[tokio::test]
async fn test_put_preserves_other_sheets() {
    let mut sheet = TestSheet::default();

    // Count original sheets
    let transactions_original = sheet.get(TRANSACTIONS).await.unwrap();
    let categories_original = sheet.get(CATEGORIES).await.unwrap();

    // Add a new sheet
    let new_data = vec![vec!["New".to_string()]];
    sheet._put("NewSheet", &new_data).await.unwrap();

    // Verify original sheets unchanged
    assert_eq!(
        sheet.get(TRANSACTIONS).await.unwrap(),
        transactions_original
    );
    assert_eq!(sheet.get(CATEGORIES).await.unwrap(), categories_original);

    // Verify new sheet exists
    assert_eq!(sheet.get("NewSheet").await.unwrap(), new_data);
}

#[tokio::test]
async fn test_concurrent_access() {
    use tokio::task;

    let sheet = TestSheet::default();

    // Spawn multiple tasks that read concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let mut sheet_clone = sheet.clone();
        let handle = task::spawn(async move {
            let sheet_name = if i % 3 == 0 {
                TRANSACTIONS
            } else if i % 3 == 1 {
                CATEGORIES
            } else {
                AUTO_CAT
            };
            sheet_clone.get(sheet_name).await.is_ok()
        });
        handles.push(handle);
    }

    // All reads should succeed
    for handle in handles {
        assert!(handle.await.unwrap());
    }
}

#[tokio::test]
async fn test_update_then_read_consistency() {
    let mut sheet = TestSheet::default();

    // Perform multiple updates
    for i in 0..5 {
        let data = vec![
            vec!["Iteration".to_string(), i.to_string()],
            vec!["Data".to_string(), format!("Value{i}")],
        ];
        sheet._put("UpdateTest", &data).await.unwrap();

        // Immediately read back
        let retrieved = sheet.get("UpdateTest").await.unwrap();
        assert_eq!(retrieved[0][1], i.to_string());
        assert_eq!(retrieved[1][1], format!("Value{i}"));
    }
}

#[tokio::test]
async fn test_realistic_transaction_data() {
    let mut sheet = TestSheet::default();

    let realistic_data = vec![
        vec![
            "".to_string(),
            "Date".to_string(),
            "Description".to_string(),
            "Category".to_string(),
            "Amount".to_string(),
            "Account".to_string(),
        ],
        vec![
            "".to_string(),
            "2025-11-22".to_string(),
            "Amazon Purchase".to_string(),
            "Shopping".to_string(),
            "-$45.67".to_string(),
            "Chase Checking".to_string(),
        ],
        vec![
            "".to_string(),
            "2025-11-21".to_string(),
            "Salary Deposit".to_string(),
            "Income".to_string(),
            "$5000.00".to_string(),
            "Chase Checking".to_string(),
        ],
    ];

    sheet._put(TRANSACTIONS, &realistic_data).await.unwrap();
    let retrieved = sheet.get(TRANSACTIONS).await.unwrap();

    assert_eq!(retrieved.len(), 3);
    assert_eq!(retrieved[1][4], "-$45.67");
    assert_eq!(retrieved[2][4], "$5000.00");
}
