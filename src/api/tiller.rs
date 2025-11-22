//! Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.

use crate::api::{Sheet, Tiller, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::model::{AutoCats, Categories, TillerData, Transactions};
use crate::Result;

/// Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.
pub(super) struct TillerImpl {
    sheet: Box<dyn Sheet + Send>,
}

impl TillerImpl {
    /// Create a new `TillerImpl` object that will use a dynamically-dispatched `sheet` to get and
    /// send its data.
    pub(super) async fn new(sheet: Box<dyn Sheet + Send>) -> Result<Self> {
        Ok(Self { sheet })
    }
}

#[async_trait::async_trait]
impl Tiller for TillerImpl {
    async fn get_data(&mut self) -> Result<TillerData> {
        // Fetch data from all three tabs
        let transactions = fetch_transactions(self.sheet.as_mut()).await?;
        let categories = fetch_categories(self.sheet.as_mut()).await?;
        let auto_cats = fetch_auto_cats(self.sheet.as_mut()).await?;

        Ok(TillerData {
            transactions,
            categories,
            auto_cats,
        })
    }
}

/// Fetches transaction data from the Transactions tab
async fn fetch_transactions(client: &mut (dyn Sheet + Send)) -> Result<Transactions> {
    let values = client.get(TRANSACTIONS).await?;
    Transactions::new(values)
}

/// Fetches category data from the Categories tab
async fn fetch_categories(client: &mut (dyn Sheet + Send)) -> Result<Categories> {
    let values = client.get(CATEGORIES).await?;
    Categories::new(values)
}

/// Fetches AutoCat data from the AutoCat tab
async fn fetch_auto_cats(client: &mut (dyn Sheet + Send)) -> Result<AutoCats> {
    let values = client.get(AUTO_CAT).await?;
    AutoCats::new(values)
}
