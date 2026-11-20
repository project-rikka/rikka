use rikka_proton_account::{
    Account, PublishedAddress, PublishedKeys, SignedKeyList, SignedKeyListItem,
};

use crate::client::KeyTransparency;
use crate::error::{Error, Result};
use crate::excluded_domains;
use crate::failure::{self, Failure};
use crate::identifier::{self, Identifier, Scope};
use crate::published::VerifiedPublicKeys;
use crate::state::PendingKeyList;
use crate::verdict::{Pending, RecipientVerdicts, Unverifiable, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExcludedDomains {
    Verify,
    Skip,
}

#[must_use]
#[derive(Debug)]
pub struct RecipientCheck<'a> {
    transparency: &'a KeyTransparency,
    email: &'a str,
    address: Option<&'a PublishedKeys>,
    catch_all: Option<&'a PublishedKeys>,
    excluded_domains: ExcludedDomains,
}

impl<'a> RecipientCheck<'a> {
    pub fn address(mut self, keys: &'a PublishedKeys) -> Self {
        self.address = Some(keys);
        self
    }

    pub fn catch_all(mut self, keys: &'a PublishedKeys) -> Self {
        self.catch_all = Some(keys);
        self
    }

    pub fn skipping_excluded_domains(mut self) -> Self {
        self.excluded_domains = ExcludedDomains::Skip;
        self
    }

    pub async fn verify(self) -> Result<RecipientVerdicts> {
        let empty = PublishedKeys::default();
        let address = self.address.unwrap_or(&empty);
        let address_verdict = self
            .transparency
            .verify_scope(self.email, Scope::Address, address, self.excluded_domains)
            .await?;
        let catch_all_verdict = if address.keys().is_empty() || self.catch_all.is_some() {
            Some(
                self.transparency
                    .verify_scope(
                        self.email,
                        Scope::CatchAll,
                        self.catch_all.unwrap_or(&empty),
                        self.excluded_domains,
                    )
                    .await?,
            )
        } else {
            None
        };
        Ok(RecipientVerdicts::new(address_verdict, catch_all_verdict))
    }
}

#[must_use]
#[derive(Debug)]
pub struct Lookup<'a> {
    transparency: &'a KeyTransparency,
    email: &'a str,
    internal_only: bool,
    excluded_domains: ExcludedDomains,
}

impl Lookup<'_> {
    pub fn internal_only(mut self) -> Self {
        self.internal_only = true;
        self
    }

    pub fn skipping_excluded_domains(mut self) -> Self {
        self.excluded_domains = ExcludedDomains::Skip;
        self
    }

    pub async fn send(self) -> Result<VerifiedPublicKeys> {
        let account = Account::with_pgp(self.transparency.api().clone(), *self.transparency.pgp());
        let mut lookup = account.public_keys(self.email);
        if self.internal_only {
            lookup = lookup.internal_only();
        }
        let keys = match lookup.send().await? {
            PublishedAddress::Keys(keys) => *keys,
            PublishedAddress::Unpublished(refusal) => {
                return Err(Error::Api(rikka_proton_api::Error::Api(refusal)));
            }
        };
        let mut check = self.transparency.check(self.email).address(keys.address());
        if let Some(catch_all) = keys.catch_all() {
            check = check.catch_all(catch_all);
        }
        if self.excluded_domains == ExcludedDomains::Skip {
            check = check.skipping_excluded_domains();
        }
        let verdicts = check.verify().await?;
        Ok(VerifiedPublicKeys::new(keys, verdicts))
    }
}

impl KeyTransparency {
    pub fn check<'a>(&'a self, email: &'a str) -> RecipientCheck<'a> {
        RecipientCheck {
            transparency: self,
            email,
            address: None,
            catch_all: None,
            excluded_domains: ExcludedDomains::Verify,
        }
    }

    pub fn lookup<'a>(&'a self, email: &'a str) -> Lookup<'a> {
        Lookup {
            transparency: self,
            email,
            internal_only: false,
            excluded_domains: ExcludedDomains::Verify,
        }
    }

    pub async fn verify_keys(
        &self,
        email: &str,
        keys: &[SignedKeyListItem],
        signed_key_list: Option<&SignedKeyList>,
    ) -> Result<Verdict> {
        failure_into_verdict(
            self.evaluate(
                email,
                Scope::Address,
                keys,
                signed_key_list,
                ExcludedDomains::Verify,
            )
            .await,
        )
    }

    async fn verify_scope(
        &self,
        email: &str,
        scope: Scope,
        published: &PublishedKeys,
        excluded_domains: ExcludedDomains,
    ) -> Result<Verdict> {
        let keys = published.signed_key_list_items();
        failure_into_verdict(
            self.evaluate(
                email,
                scope,
                &keys,
                published.signed_key_list(),
                excluded_domains,
            )
            .await,
        )
    }

    async fn evaluate(
        &self,
        email: &str,
        scope: Scope,
        keys: &[SignedKeyListItem],
        signed_key_list: Option<&SignedKeyList>,
        excluded_domains: ExcludedDomains,
    ) -> Result<Verdict> {
        self.ensure_available().await?;
        let unsigned = signed_key_list.and_then(SignedKeyList::signature).is_none();
        if unsigned
            && excluded_domains == ExcludedDomains::Skip
            && excluded_domains::is_excluded(&identifier::email_domain(email)?)
        {
            return Ok(Verdict::Unverifiable(Unverifiable::ExcludedDomain));
        }
        if signed_key_list.is_some_and(|list| list.revision() == 0) {
            return Err(Failure::RevisionZero.into());
        }
        if let Some(data) = signed_key_list.and_then(SignedKeyList::data) {
            failure::parse_key_list(data)?
                .compare(keys)
                .map_err(Failure::from)?;
        }

        let identifier = Identifier::for_scope(email, scope)?;

        let Some(signed_key_list) = signed_key_list else {
            let (epoch, proofs) = self.proofs_in_latest(&identifier, &[1]).await?;
            for proof in &proofs {
                proof.verify_absence_of_all(self.realm(), epoch.tree_hash(), &identifier)?;
            }
            return Ok(Verdict::Unverifiable(Unverifiable::Absent));
        };

        if !signed_key_list.is_included() {
            let expected = signed_key_list
                .expected_min_epoch_id()
                .ok_or(Failure::MissingMinEpoch)?;
            let content = signed_key_list.content().ok_or(Failure::MissingContent)?;
            self.add_pending(
                PendingKeyList::new(
                    self.now(),
                    email,
                    content,
                    signed_key_list.revision(),
                    expected,
                )
                .with_scope(scope),
            );
            return Ok(Verdict::Pending(
                if signed_key_list.obsolescence_token().is_some() {
                    Pending::Obsolescence
                } else {
                    Pending::Keys
                },
            ));
        }

        let revision = signed_key_list.revision();
        let next = revision.saturating_add(1);
        let (epoch, proofs) = self
            .proofs_in_latest(&identifier, &[revision, next])
            .await?;
        let [proof, next_proof] = proofs.as_slice() else {
            return Err(Failure::IncompleteSignedKeyList.into());
        };
        next_proof.verify_absence(self.realm(), epoch.tree_hash(), &identifier, next)?;
        if unsigned {
            proof.verify_obsolescence(
                self.realm(),
                epoch.tree_hash(),
                &identifier,
                signed_key_list,
            )?;
            Ok(Verdict::Unverifiable(Unverifiable::Obsolete))
        } else {
            proof.verify_existence(
                self.realm(),
                epoch.tree_hash(),
                &identifier,
                signed_key_list,
            )?;
            Ok(Verdict::Verified)
        }
    }
}

pub(crate) fn failure_into_verdict(result: Result<Verdict>) -> Result<Verdict> {
    match result {
        Err(Error::Failed(failure)) => Ok(Verdict::Failed(failure)),
        result => result,
    }
}
