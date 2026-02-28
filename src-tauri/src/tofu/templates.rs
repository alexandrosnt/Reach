use std::collections::HashMap;

use crate::tofu::types::{
    ProjectTemplate, TofuOutput, TofuProviderConfig, TofuResourceConfig, TofuVariable, TofuVarType,
};

/// Return the static project template catalog.
pub fn get_project_templates() -> Vec<ProjectTemplate> {
    vec![
        aws_vpc_ec2_template(),
        docker_nginx_template(),
        aws_static_site_template(),
        k8s_app_template(),
        aws_rds_template(),
    ]
}

fn aws_vpc_ec2_template() -> ProjectTemplate {
    ProjectTemplate {
        id: "aws_vpc_ec2".into(),
        name: "AWS VPC with EC2".into(),
        description: "A VPC with a public subnet and an EC2 instance. Great starting point for AWS.".into(),
        category: "AWS".into(),
        providers: vec![TofuProviderConfig {
            provider_id: "aws".into(),
            source: "hashicorp/aws".into(),
            version: ">= 5.0".into(),
            fields: HashMap::from([("region".into(), serde_json::json!("us-east-1"))]),
        }],
        variables: vec![
            TofuVariable {
                name: "instance_type".into(),
                var_type: TofuVarType::String,
                description: "EC2 instance type".into(),
                default_value: Some("t3.micro".into()),
                sensitive: false,
            },
            TofuVariable {
                name: "ami_id".into(),
                var_type: TofuVarType::String,
                description: "AMI ID for the EC2 instance".into(),
                default_value: Some("ami-0c02fb55956c7d316".into()),
                sensitive: false,
            },
        ],
        resources: vec![
            TofuResourceConfig {
                id: "tpl-vpc".into(),
                resource_type: "aws_vpc".into(),
                logical_name: "main".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("cidr_block".into(), serde_json::json!("10.0.0.0/16")),
                    ("tags_name".into(), serde_json::json!("main-vpc")),
                ]),
            },
            TofuResourceConfig {
                id: "tpl-subnet".into(),
                resource_type: "aws_subnet".into(),
                logical_name: "public".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("vpc_id".into(), serde_json::json!("aws_vpc.main.id")),
                    ("cidr_block".into(), serde_json::json!("10.0.1.0/24")),
                    ("map_public_ip_on_launch".into(), serde_json::json!(true)),
                    ("tags_name".into(), serde_json::json!("public-subnet")),
                ]),
            },
            TofuResourceConfig {
                id: "tpl-instance".into(),
                resource_type: "aws_instance".into(),
                logical_name: "web".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("ami".into(), serde_json::json!("var.ami_id")),
                    ("instance_type".into(), serde_json::json!("var.instance_type")),
                    ("subnet_id".into(), serde_json::json!("aws_subnet.public.id")),
                    ("tags_name".into(), serde_json::json!("web-server")),
                ]),
            },
        ],
        outputs: vec![
            TofuOutput {
                name: "instance_public_ip".into(),
                value: "aws_instance.web.public_ip".into(),
                description: "Public IP of the EC2 instance".into(),
                sensitive: false,
            },
            TofuOutput {
                name: "vpc_id".into(),
                value: "aws_vpc.main.id".into(),
                description: "ID of the VPC".into(),
                sensitive: false,
            },
        ],
    }
}

fn docker_nginx_template() -> ProjectTemplate {
    ProjectTemplate {
        id: "docker_nginx".into(),
        name: "Docker Nginx".into(),
        description: "A Docker container running Nginx with configurable port mapping.".into(),
        category: "Docker".into(),
        providers: vec![TofuProviderConfig {
            provider_id: "docker".into(),
            source: "kreuzwerker/docker".into(),
            version: ">= 3.0".into(),
            fields: HashMap::new(),
        }],
        variables: vec![TofuVariable {
            name: "external_port".into(),
            var_type: TofuVarType::Number,
            description: "External port to expose Nginx on".into(),
            default_value: Some("8080".into()),
            sensitive: false,
        }],
        resources: vec![
            TofuResourceConfig {
                id: "tpl-image".into(),
                resource_type: "docker_image".into(),
                logical_name: "nginx".into(),
                provider_id: "docker".into(),
                fields: HashMap::from([("name".into(), serde_json::json!("nginx:latest"))]),
            },
            TofuResourceConfig {
                id: "tpl-container".into(),
                resource_type: "docker_container".into(),
                logical_name: "web".into(),
                provider_id: "docker".into(),
                fields: HashMap::from([
                    ("name".into(), serde_json::json!("nginx-web")),
                    ("image".into(), serde_json::json!("docker_image.nginx.image_id")),
                    ("ports_internal".into(), serde_json::json!("80")),
                    ("ports_external".into(), serde_json::json!("var.external_port")),
                ]),
            },
        ],
        outputs: vec![TofuOutput {
            name: "container_id".into(),
            value: "docker_container.web.id".into(),
            description: "ID of the Nginx container".into(),
            sensitive: false,
        }],
    }
}

fn aws_static_site_template() -> ProjectTemplate {
    ProjectTemplate {
        id: "aws_static_site".into(),
        name: "AWS Static Website".into(),
        description: "An S3 bucket configured for static website hosting.".into(),
        category: "AWS".into(),
        providers: vec![TofuProviderConfig {
            provider_id: "aws".into(),
            source: "hashicorp/aws".into(),
            version: ">= 5.0".into(),
            fields: HashMap::from([("region".into(), serde_json::json!("us-east-1"))]),
        }],
        variables: vec![TofuVariable {
            name: "bucket_name".into(),
            var_type: TofuVarType::String,
            description: "Name of the S3 bucket".into(),
            default_value: None,
            sensitive: false,
        }],
        resources: vec![TofuResourceConfig {
            id: "tpl-bucket".into(),
            resource_type: "aws_s3_bucket".into(),
            logical_name: "site".into(),
            provider_id: "aws".into(),
            fields: HashMap::from([("bucket".into(), serde_json::json!("var.bucket_name"))]),
        }],
        outputs: vec![
            TofuOutput {
                name: "bucket_arn".into(),
                value: "aws_s3_bucket.site.arn".into(),
                description: "ARN of the S3 bucket".into(),
                sensitive: false,
            },
            TofuOutput {
                name: "bucket_domain".into(),
                value: "aws_s3_bucket.site.bucket_regional_domain_name".into(),
                description: "Regional domain name of the bucket".into(),
                sensitive: false,
            },
        ],
    }
}

fn k8s_app_template() -> ProjectTemplate {
    ProjectTemplate {
        id: "k8s_app".into(),
        name: "Kubernetes App".into(),
        description: "A Kubernetes namespace, deployment, and service for a containerized app.".into(),
        category: "Kubernetes".into(),
        providers: vec![TofuProviderConfig {
            provider_id: "kubernetes".into(),
            source: "hashicorp/kubernetes".into(),
            version: ">= 2.0".into(),
            fields: HashMap::new(),
        }],
        variables: vec![
            TofuVariable {
                name: "app_name".into(),
                var_type: TofuVarType::String,
                description: "Name of the application".into(),
                default_value: Some("myapp".into()),
                sensitive: false,
            },
            TofuVariable {
                name: "container_image".into(),
                var_type: TofuVarType::String,
                description: "Container image to deploy".into(),
                default_value: Some("nginx:latest".into()),
                sensitive: false,
            },
            TofuVariable {
                name: "replicas".into(),
                var_type: TofuVarType::Number,
                description: "Number of replicas".into(),
                default_value: Some("2".into()),
                sensitive: false,
            },
        ],
        resources: vec![
            TofuResourceConfig {
                id: "tpl-ns".into(),
                resource_type: "kubernetes_namespace".into(),
                logical_name: "app".into(),
                provider_id: "kubernetes".into(),
                fields: HashMap::from([("name".into(), serde_json::json!("var.app_name"))]),
            },
            TofuResourceConfig {
                id: "tpl-deploy".into(),
                resource_type: "kubernetes_deployment".into(),
                logical_name: "app".into(),
                provider_id: "kubernetes".into(),
                fields: HashMap::from([
                    ("container_name".into(), serde_json::json!("var.app_name")),
                    ("container_image".into(), serde_json::json!("var.container_image")),
                    ("container_port".into(), serde_json::json!("80")),
                    ("replicas".into(), serde_json::json!("var.replicas")),
                ]),
            },
            TofuResourceConfig {
                id: "tpl-svc".into(),
                resource_type: "kubernetes_service".into(),
                logical_name: "app".into(),
                provider_id: "kubernetes".into(),
                fields: HashMap::from([
                    ("type".into(), serde_json::json!("LoadBalancer")),
                    ("port".into(), serde_json::json!("80")),
                    ("target_port".into(), serde_json::json!("80")),
                    ("selector_app".into(), serde_json::json!("var.app_name")),
                ]),
            },
        ],
        outputs: vec![TofuOutput {
            name: "namespace".into(),
            value: "kubernetes_namespace.app.metadata[0].name".into(),
            description: "Name of the Kubernetes namespace".into(),
            sensitive: false,
        }],
    }
}

fn aws_rds_template() -> ProjectTemplate {
    ProjectTemplate {
        id: "aws_rds".into(),
        name: "AWS RDS Database".into(),
        description: "A VPC with a subnet and an RDS database instance.".into(),
        category: "AWS".into(),
        providers: vec![TofuProviderConfig {
            provider_id: "aws".into(),
            source: "hashicorp/aws".into(),
            version: ">= 5.0".into(),
            fields: HashMap::from([("region".into(), serde_json::json!("us-east-1"))]),
        }],
        variables: vec![
            TofuVariable {
                name: "db_username".into(),
                var_type: TofuVarType::String,
                description: "Database master username".into(),
                default_value: Some("admin".into()),
                sensitive: false,
            },
            TofuVariable {
                name: "db_password".into(),
                var_type: TofuVarType::String,
                description: "Database master password".into(),
                default_value: None,
                sensitive: true,
            },
        ],
        resources: vec![
            TofuResourceConfig {
                id: "tpl-vpc".into(),
                resource_type: "aws_vpc".into(),
                logical_name: "db_vpc".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("cidr_block".into(), serde_json::json!("10.0.0.0/16")),
                    ("tags_name".into(), serde_json::json!("db-vpc")),
                ]),
            },
            TofuResourceConfig {
                id: "tpl-subnet".into(),
                resource_type: "aws_subnet".into(),
                logical_name: "db_subnet".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("vpc_id".into(), serde_json::json!("aws_vpc.db_vpc.id")),
                    ("cidr_block".into(), serde_json::json!("10.0.1.0/24")),
                    ("tags_name".into(), serde_json::json!("db-subnet")),
                ]),
            },
            TofuResourceConfig {
                id: "tpl-db".into(),
                resource_type: "aws_db_instance".into(),
                logical_name: "main".into(),
                provider_id: "aws".into(),
                fields: HashMap::from([
                    ("allocated_storage".into(), serde_json::json!("20")),
                    ("engine".into(), serde_json::json!("mysql")),
                    ("engine_version".into(), serde_json::json!("8.0")),
                    ("instance_class".into(), serde_json::json!("db.t3.micro")),
                    ("username".into(), serde_json::json!("var.db_username")),
                    ("password".into(), serde_json::json!("var.db_password")),
                    ("skip_final_snapshot".into(), serde_json::json!(true)),
                ]),
            },
        ],
        outputs: vec![
            TofuOutput {
                name: "db_endpoint".into(),
                value: "aws_db_instance.main.endpoint".into(),
                description: "Connection endpoint of the RDS instance".into(),
                sensitive: false,
            },
            TofuOutput {
                name: "db_arn".into(),
                value: "aws_db_instance.main.arn".into(),
                description: "ARN of the RDS instance".into(),
                sensitive: false,
            },
        ],
    }
}
